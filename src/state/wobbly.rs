use smithay::utils::{Logical, Point, Rectangle, Size};

/// A single mass point in the wobbly spring grid.
#[derive(Clone, Copy, Debug)]
pub struct WobblyPoint {
    /// Ideal rest position in target window space.
    pub target: Point<f64, Logical>,
    /// Current simulated position.
    pub pos: Point<f64, Logical>,
    /// Current velocity.
    pub vel: Point<f64, Logical>,
    /// Current acceleration.
    pub acc: Point<f64, Logical>,
    /// Whether this point is currently constrained to the cursor drag.
    pub constraint: bool,
}

impl WobblyPoint {
    fn new(pos: Point<f64, Logical>) -> Self {
        Self {
            target: pos,
            pos,
            vel: Point::from((0.0, 0.0)),
            acc: Point::from((0.0, 0.0)),
            constraint: false,
        }
    }
}

/// 4x4 mass-spring-damper grid for KDE-style wobbly windows.
#[derive(Clone, Debug)]
pub struct WobblyState {
    pub points: [WobblyPoint; 16],
    pub target_rect: Rectangle<f64, Logical>,
    pub grab_point: Point<f64, Logical>,
    pub is_dragging: bool,
    /// Damping threshold below which motion is considered settled.
    pub settled: bool,
    /// Whether the window is magnetically snapped along the X axis.
    pub snapped_x: bool,
    /// Whether the window is magnetically snapped along the Y axis.
    pub snapped_y: bool,
}

impl WobblyState {
    pub const GRID_SIZE: usize = 4;

    /// Initialize a wobbly spring grid for a window starting a move grab.
    pub fn new(rect: Rectangle<f64, Logical>, grab_point: Point<f64, Logical>) -> Self {
        let mut points = [WobblyPoint::new(Point::from((0.0, 0.0))); 16];
        let w = rect.size.w.max(1.0);
        let h = rect.size.h.max(1.0);
        let dx = w / 3.0;
        let dy = h / 3.0;

        for j in 0..Self::GRID_SIZE {
            for i in 0..Self::GRID_SIZE {
                let p = Point::from((rect.loc.x + (i as f64) * dx, rect.loc.y + (j as f64) * dy));
                points[j * Self::GRID_SIZE + i] = WobblyPoint::new(p);
            }
        }

        // Find the grid vertex closest to the grab point to pin as constraint.
        let u = ((grab_point.x - rect.loc.x) / w).clamp(0.0, 1.0);
        let v = ((grab_point.y - rect.loc.y) / h).clamp(0.0, 1.0);
        let gi = (u * 3.0).round().clamp(0.0, 3.0) as usize;
        let gj = (v * 3.0).round().clamp(0.0, 3.0) as usize;
        let grab_idx = gj * Self::GRID_SIZE + gi;
        points[grab_idx].constraint = true;

        Self {
            points,
            target_rect: rect,
            grab_point,
            is_dragging: true,
            settled: false,
            snapped_x: false,
            snapped_y: false,
        }
    }

    /// Update target window rect and cursor position during interactive drag,
    /// with optional magnetic snap status on X and Y axes.
    pub fn on_move(
        &mut self,
        new_rect: Rectangle<f64, Logical>,
        cursor: Point<f64, Logical>,
        snapped_x: bool,
        snapped_y: bool,
    ) {
        self.target_rect = new_rect;
        self.grab_point = cursor;
        self.is_dragging = true;
        self.settled = false;
        self.snapped_x = snapped_x;
        self.snapped_y = snapped_y;

        let w = new_rect.size.w.max(1.0);
        let h = new_rect.size.h.max(1.0);
        let dx = w / 3.0;
        let dy = h / 3.0;

        for j in 0..Self::GRID_SIZE {
            for i in 0..Self::GRID_SIZE {
                let idx = j * Self::GRID_SIZE + i;
                let target = Point::from((new_rect.loc.x + (i as f64) * dx, new_rect.loc.y + (j as f64) * dy));
                self.points[idx].target = target;
            }
        }

        // If snapped along an axis, damp velocities along that axis so the window
        // cleanly sticks to the magnetic border without lingering oscillation.
        if snapped_x {
            for pt in &mut self.points {
                pt.vel.x *= 0.5;
            }
        }
        if snapped_y {
            for pt in &mut self.points {
                pt.vel.y *= 0.5;
            }
        }
    }

    /// Release the grab constraint when mouse button or touch lifts.
    pub fn on_release(&mut self) {
        self.is_dragging = false;
        self.snapped_x = false;
        self.snapped_y = false;
        let w = self.target_rect.size.w.max(1.0);
        let h = self.target_rect.size.h.max(1.0);
        let dx = w / 3.0;
        let dy = h / 3.0;

        for j in 0..Self::GRID_SIZE {
            for i in 0..Self::GRID_SIZE {
                let idx = j * Self::GRID_SIZE + i;
                self.points[idx].constraint = false;
                self.points[idx].target = Point::from((
                    self.target_rect.loc.x + (i as f64) * dx,
                    self.target_rect.loc.y + (j as f64) * dy,
                ));
            }
        }
    }

    /// Advance the spring simulation by elapsed time `dt_sec`.
    /// Returns `true` if the window is still wobbly, or `false` once oscillations have settled.
    pub fn step(
        &mut self,
        dt_sec: f64,
        stiffness: f64,
        drag: f64,
        move_factor: f64,
    ) -> bool {
        if self.settled {
            return false;
        }

        // Limit dt to avoid numerical instability on frame drops.
        let dt = dt_sec.clamp(0.001, 0.05);
        let w = self.target_rect.size.w.max(1.0);
        let h = self.target_rect.size.h.max(1.0);
        let rest_dx = w / 3.0;
        let rest_dy = h / 3.0;

        // Compute spring forces and accelerations for each vertex.
        for j in 0..Self::GRID_SIZE {
            for i in 0..Self::GRID_SIZE {
                let idx = j * Self::GRID_SIZE + i;
                let pos = self.points[idx].pos;
                let mut force_x = 0.0;
                let mut force_y = 0.0;
                let mut neighbor_count = 0.0;

                // Left neighbor
                if i > 0 {
                    let left = self.points[idx - 1].pos;
                    force_x += ((left.x + rest_dx) - pos.x) * stiffness;
                    force_y += (left.y - pos.y) * stiffness;
                    neighbor_count += 1.0;
                }
                // Right neighbor
                if i < Self::GRID_SIZE - 1 {
                    let right = self.points[idx + 1].pos;
                    force_x += ((right.x - rest_dx) - pos.x) * stiffness;
                    force_y += (right.y - pos.y) * stiffness;
                    neighbor_count += 1.0;
                }
                // Top neighbor
                if j > 0 {
                    let top = self.points[idx - Self::GRID_SIZE].pos;
                    force_x += (top.x - pos.x) * stiffness;
                    force_y += ((top.y + rest_dy) - pos.y) * stiffness;
                    neighbor_count += 1.0;
                }
                // Bottom neighbor
                if j < Self::GRID_SIZE - 1 {
                    let bottom = self.points[idx + Self::GRID_SIZE].pos;
                    force_x += (bottom.x - pos.x) * stiffness;
                    force_y += ((bottom.y - rest_dy) - pos.y) * stiffness;
                    neighbor_count += 1.0;
                }

                // Restorative pull towards ideal home target.
                let home = self.points[idx].target;
                let home_stiffness = if self.points[idx].constraint {
                    stiffness * 2.5
                } else {
                    stiffness
                };
                force_x += (home.x - pos.x) * home_stiffness;
                force_y += (home.y - pos.y) * home_stiffness;
                neighbor_count += 1.0;

                if neighbor_count > 0.0 {
                    self.points[idx].acc = Point::from((
                        force_x / neighbor_count,
                        force_y / neighbor_count,
                    ));
                }
            }
        }

        // Integrate velocity and position.
        let mut max_vel: f64 = 0.0;
        let mut max_disp: f64 = 0.0;

        for pt in &mut self.points {
            let acc = pt.acc;
            let mut drag_x = drag;
            let mut drag_y = drag;
            if self.snapped_x {
                drag_x *= 0.6;
            }
            if self.snapped_y {
                drag_y *= 0.6;
            }

            pt.vel.x = acc.x * (dt * 60.0) + pt.vel.x * drag_x;
            pt.vel.y = acc.y * (dt * 60.0) + pt.vel.y * drag_y;

            // Cap velocities to prevent runaway explosion.
            let v_cap = 1500.0;
            pt.vel.x = pt.vel.x.clamp(-v_cap, v_cap);
            pt.vel.y = pt.vel.y.clamp(-v_cap, v_cap);

            pt.pos.x += pt.vel.x * (dt * 60.0) * move_factor;
            pt.pos.y += pt.vel.y * (dt * 60.0) * move_factor;

            let v = pt.vel.x.abs().max(pt.vel.y.abs());
            let d = (pt.pos.x - pt.target.x).abs().max((pt.pos.y - pt.target.y).abs());
            if v > max_vel {
                max_vel = v;
            }
            if d > max_disp {
                max_disp = d;
            }
        }

        // If not dragging and motion has dampened below threshold, snap to home.
        if !self.is_dragging && max_vel < 0.2 && max_disp < 0.3 {
            for pt in &mut self.points {
                pt.pos = pt.target;
                pt.vel = Point::from((0.0, 0.0));
            }
            self.settled = true;
            return false;
        }

        true
    }

    /// Calculate visual location and size of the wobbly window.
    pub fn visual_rect(&self) -> Rectangle<f64, Logical> {
        let mut center_x = 0.0;
        let mut center_y = 0.0;
        for pt in &self.points {
            center_x += pt.pos.x;
            center_y += pt.pos.y;
        }
        center_x /= 16.0;
        center_y /= 16.0;

        // Calculate average width across rows.
        let mut avg_w = 0.0;
        for j in 0..Self::GRID_SIZE {
            let left = self.points[j * Self::GRID_SIZE].pos.x;
            let right = self.points[j * Self::GRID_SIZE + 3].pos.x;
            avg_w += (right - left).abs();
        }
        avg_w /= 4.0;

        // Calculate average height across columns.
        let mut avg_h = 0.0;
        for i in 0..Self::GRID_SIZE {
            let top = self.points[i].pos.y;
            let bottom = self.points[3 * Self::GRID_SIZE + i].pos.y;
            avg_h += (bottom - top).abs();
        }
        avg_h /= 4.0;

        let w = avg_w.max(20.0);
        let h = avg_h.max(20.0);
        let mut loc = Point::from((center_x - w / 2.0, center_y - h / 2.0));

        // When magnetically snapped, firmly anchor the snapped axis to the target
        // placement while allowing the window body to settle with elastic bounce.
        if self.snapped_x {
            loc.x = self.target_rect.loc.x;
        }
        if self.snapped_y {
            loc.y = self.target_rect.loc.y;
        }

        Rectangle::new(loc, Size::from((w, h)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wobbly_initialization() {
        let rect = Rectangle::new(Point::from((100.0, 100.0)), Size::from((300.0, 300.0)));
        let grab = Point::from((150.0, 120.0));
        let wobbly = WobblyState::new(rect, grab);

        assert_eq!(wobbly.points.len(), 16);
        assert!(wobbly.points.iter().any(|p| p.constraint));
        let vr = wobbly.visual_rect();
        assert!((vr.loc.x - rect.loc.x).abs() < 1.0);
        assert!((vr.loc.y - rect.loc.y).abs() < 1.0);
        assert!((vr.size.w - rect.size.w).abs() < 1.0);
        assert!((vr.size.h - rect.size.h).abs() < 1.0);
    }

    #[test]
    fn wobbly_drag_and_settle() {
        let rect1 = Rectangle::new(Point::from((100.0, 100.0)), Size::from((300.0, 300.0)));
        let grab1 = Point::from((150.0, 150.0));
        let mut wobbly = WobblyState::new(rect1, grab1);

        // Move window to new position
        let rect2 = Rectangle::new(Point::from((200.0, 200.0)), Size::from((300.0, 300.0)));
        let grab2 = Point::from((250.0, 250.0));
        wobbly.on_move(rect2, grab2, false, false);

        // Advance simulation for 5 frames while dragging
        for _ in 0..5 {
            wobbly.step(0.016, 0.15, 0.85, 0.10);
        }

        // Release the window
        wobbly.on_release();
        assert!(!wobbly.is_dragging);

        // Simulate until settled
        let mut settled = false;
        for _ in 0..300 {
            if !wobbly.step(0.016, 0.15, 0.85, 0.10) {
                settled = true;
                break;
            }
        }
        assert!(settled, "Wobbly window must settle after release");
        let vr = wobbly.visual_rect();
        assert!((vr.loc.x - rect2.loc.x).abs() < 0.5);
        assert!((vr.loc.y - rect2.loc.y).abs() < 0.5);
    }

    #[test]
    fn wobbly_magnetic_snapping_anchors_axis() {
        let rect1 = Rectangle::new(Point::from((100.0, 100.0)), Size::from((300.0, 300.0)));
        let grab1 = Point::from((150.0, 150.0));
        let mut wobbly = WobblyState::new(rect1, grab1);

        // Window snapped along X (e.g. docked to left neighbor at x = 500)
        let rect2 = Rectangle::new(Point::from((500.0, 200.0)), Size::from((300.0, 300.0)));
        // Cursor pulled 20px further right before breaking snap
        let grab2 = Point::from((570.0, 250.0));
        wobbly.on_move(rect2, grab2, true, false);

        // Advance several physics frames
        for _ in 0..10 {
            wobbly.step(0.016, 0.15, 0.85, 0.10);
        }

        let vr = wobbly.visual_rect();
        // Snapped X axis must remain firmly anchored to target_rect.loc.x
        assert_eq!(vr.loc.x, 500.0, "Snapped axis must not drift with cursor");
    }

    #[test]
    fn wobbly_window_animations_lifecycle() {
        use crate::state::window_animation::{AnimSpace, WindowAnimations};
        let mut anims = WindowAnimations::default();
        let id = driftwm::stage::ElementId(42);
        let rect = Rectangle::new(Point::from((50.0, 50.0)), Size::from((400.0, 300.0)));
        let grab = Point::from((100.0, 80.0));

        anims.start_wobbly(id, rect, grab);
        assert!(anims.is_wobbly(id));

        // Scoping entries should return canvas animation space with visual rect
        let scopes = anims.scoping_entries();
        assert_eq!(scopes.len(), 1);
        assert_eq!(scopes[0].0, id);
        assert!(matches!(scopes[0].1, Some((AnimSpace::Canvas, _))));

        // Update movement
        let new_rect = Rectangle::new(Point::from((150.0, 150.0)), Size::from((400.0, 300.0)));
        let new_cursor = Point::from((200.0, 180.0));
        anims.update_wobbly(id, new_rect, new_cursor, false, false);

        // Tick several times to advance physics simulation
        let now = std::time::Instant::now();
        for _ in 0..10 {
            anims.tick_entry(id, new_rect.loc, new_rect.size, 0.5, now, true, 0.15, 0.85, 0.10);
        }

        // Release drag
        anims.release_wobbly(id);

        // Tick until oscillation settles and animation entry is removed
        let mut settled = false;
        for _ in 0..300 {
            let keep = anims.tick_entry(id, new_rect.loc, new_rect.size, 0.5, now, true, 0.15, 0.85, 0.10);
            if !keep {
                anims.remove(id);
                settled = true;
                break;
            }
        }
        assert!(settled, "Animation should settle after release");
        assert!(!anims.is_wobbly(id), "Animation should be removed once settled");
    }
}

