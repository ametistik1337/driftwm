use smithay::utils::{Logical, Point, Rectangle, Size};

/// 2D point pair directly matching KWin's `WobblyWindowsEffect::Pair`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pair {
    pub x: f64,
    pub y: f64,
}

impl Pair {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Clamps vector components to min and max bounds, exactly as KWin `fixVectorBounds`.
fn fix_vector_bounds(vec: &mut Pair, min: f64, max: f64) {
    if vec.x.abs() < min {
        vec.x = 0.0;
    } else if vec.x.abs() > max {
        vec.x = if vec.x > 0.0 { max } else { -max };
    }

    if vec.y.abs() < min {
        vec.y = 0.0;
    } else if vec.y.abs() > max {
        vec.y = if vec.y > 0.0 { max } else { -max };
    }
}

/// Evaluates a point on the 4x4 bicubic Bezier surface for normalized UV in [0, 1]²,
/// matching `KWin::WobblyWindowsEffect::computeBezierPoint`.
pub fn compute_bezier_point(positions: &[Pair; 16], point: Pair) -> Pair {
    let tx = point.x.clamp(0.0, 1.0);
    let ty = point.y.clamp(0.0, 1.0);

    let mut px = [0.0; 4];
    px[0] = (1.0 - tx) * (1.0 - tx) * (1.0 - tx);
    px[1] = 3.0 * (1.0 - tx) * (1.0 - tx) * tx;
    px[2] = 3.0 * (1.0 - tx) * tx * tx;
    px[3] = tx * tx * tx;

    let mut py = [0.0; 4];
    py[0] = (1.0 - ty) * (1.0 - ty) * (1.0 - ty);
    py[1] = 3.0 * (1.0 - ty) * (1.0 - ty) * ty;
    py[2] = 3.0 * (1.0 - ty) * ty * ty;
    py[3] = ty * ty * ty;

    let mut res = Pair::new(0.0, 0.0);
    for j in 0..4 {
        for i in 0..4 {
            res.x += px[i] * py[j] * positions[i + j * 4].x;
            res.y += px[i] * py[j] * positions[i + j * 4].y;
        }
    }
    res
}

/// 4x4 mass-spring-damper grid for KDE-style wobbly windows,
/// direct port from `KWin::WobblyWindowsEffect`.
#[derive(Clone, Debug)]
pub struct WobblyState {
    pub origin: [Pair; 16],
    pub position: [Pair; 16],
    pub velocity: [Pair; 16],
    pub acceleration: [Pair; 16],
    pub buffer: [Pair; 16],
    pub constraint: [bool; 16],

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
    #[allow(dead_code)]
    pub const GRID_SIZE: usize = 4;
    #[allow(dead_code)]
    pub const COUNT: usize = 16;

    pub const MIN_VELOCITY: f64 = 0.0;
    pub const MAX_VELOCITY: f64 = 1000.0;
    pub const STOP_VELOCITY: f64 = 0.5;

    pub const MIN_ACCELERATION: f64 = 0.0;
    pub const MAX_ACCELERATION: f64 = 1000.0;
    pub const STOP_ACCELERATION: f64 = 0.5;

    /// Initialize a wobbly spring grid for a window starting a move grab,
    /// matching `KWin::WobblyWindowsEffect::initWobblyInfo` and `startMovedResized`.
    pub fn new(rect: Rectangle<f64, Logical>, grab_point: Point<f64, Logical>) -> Self {
        let mut origin = [Pair::default(); 16];
        let mut position = [Pair::default(); 16];
        let velocity = [Pair::default(); 16];
        let acceleration = [Pair::default(); 16];
        let buffer = [Pair::default(); 16];
        let mut constraint = [false; 16];

        let w = rect.size.w.max(1.0);
        let h = rect.size.h.max(1.0);
        let x_increment = w / 3.0;
        let y_increment = h / 3.0;

        let mut init_val = Pair::new(rect.loc.x, rect.loc.y);
        for j in 0..4 {
            for i in 0..4 {
                let idx = j * 4 + i;
                origin[idx] = init_val;
                position[idx] = init_val;
                if i != 2 {
                    init_val.x += x_increment;
                } else {
                    init_val.x = w + rect.loc.x;
                }
            }
            init_val.x = rect.loc.x;
            if j != 2 {
                init_val.y += y_increment;
            } else {
                init_val.y = h + rect.loc.y;
            }
        }

        // Find the picked grid vertex closest to the cursor grab point, matching KWin logic.
        let indx = (((grab_point.x - rect.loc.x) / x_increment) + 0.5).floor() as isize;
        let indy = (((grab_point.y - rect.loc.y) / y_increment) + 0.5).floor() as isize;
        let picked_index = (indy * 4 + indx).clamp(0, 15) as usize;
        constraint[picked_index] = true;

        Self {
            origin,
            position,
            velocity,
            acceleration,
            buffer,
            constraint,
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
        let x_increment = w / 3.0;
        let y_increment = h / 3.0;

        let mut init_val = Pair::new(new_rect.loc.x, new_rect.loc.y);
        for j in 0..4 {
            for i in 0..4 {
                let idx = j * 4 + i;
                self.origin[idx] = init_val;
                if i != 2 {
                    init_val.x += x_increment;
                } else {
                    init_val.x = w + new_rect.loc.x;
                }
            }
            init_val.x = new_rect.loc.x;
            if j != 2 {
                init_val.y += y_increment;
            } else {
                init_val.y = h + new_rect.loc.y;
            }
        }

        if snapped_x {
            for v in &mut self.velocity {
                v.x *= 0.5;
            }
        }
        if snapped_y {
            for v in &mut self.velocity {
                v.y *= 0.5;
            }
        }
    }

    /// Release grab constraint on mouse/touch button release, letting the window oscillate to settle.
    pub fn on_release(&mut self) {
        self.is_dragging = false;
        self.snapped_x = false;
        self.snapped_y = false;
        for c in &mut self.constraint {
            *c = false;
        }
    }

    /// 8-neighborhood ring linear mean blur filter for vectors,
    /// direct 1-to-1 port of `KWin::WobblyWindowsEffect::heightRingLinearMean`.
    fn height_ring_linear_mean(data: &mut [Pair; 16], buffer: &mut [Pair; 16]) {
        // Corners: (3 neighbors + 3 * self) / 6.0
        // Top-left (0)
        buffer[0] = Pair::new(
            (data[1].x + data[4].x + data[5].x + 3.0 * data[0].x) / 6.0,
            (data[1].y + data[4].y + data[5].y + 3.0 * data[0].y) / 6.0,
        );

        // Top-right (3)
        buffer[3] = Pair::new(
            (data[2].x + data[7].x + data[6].x + 3.0 * data[3].x) / 6.0,
            (data[2].y + data[7].y + data[6].y + 3.0 * data[3].y) / 6.0,
        );

        // Bottom-left (12)
        buffer[12] = Pair::new(
            (data[13].x + data[8].x + data[9].x + 3.0 * data[12].x) / 6.0,
            (data[13].y + data[8].y + data[9].y + 3.0 * data[12].y) / 6.0,
        );

        // Bottom-right (15)
        buffer[15] = Pair::new(
            (data[14].x + data[11].x + data[10].x + 3.0 * data[15].x) / 6.0,
            (data[14].y + data[11].y + data[10].y + 3.0 * data[15].y) / 6.0,
        );

        // Top border (1..2): (5 neighbors + 5 * self) / 10.0
        for i in 1..3 {
            buffer[i] = Pair::new(
                (data[i - 1].x + data[i + 1].x + data[i + 4].x + data[i + 3].x + data[i + 5].x + 5.0 * data[i].x) / 10.0,
                (data[i - 1].y + data[i + 1].y + data[i + 4].y + data[i + 3].y + data[i + 5].y + 5.0 * data[i].y) / 10.0,
            );
        }

        // Bottom border (13..14): (5 neighbors + 5 * self) / 10.0
        for i in 13..15 {
            buffer[i] = Pair::new(
                (data[i - 1].x + data[i + 1].x + data[i - 4].x + data[i - 5].x + data[i - 3].x + 5.0 * data[i].x) / 10.0,
                (data[i - 1].y + data[i + 1].y + data[i - 4].y + data[i - 5].y + data[i - 3].y + 5.0 * data[i].y) / 10.0,
            );
        }

        // Left border (j = 1..2, i = j * 4): (5 neighbors + 5 * self) / 10.0
        for j in 1..3 {
            let i = j * 4;
            buffer[i] = Pair::new(
                (data[i + 1].x + data[i - 4].x + data[i + 4].x + data[i - 3].x + data[i + 5].x + 5.0 * data[i].x) / 10.0,
                (data[i + 1].y + data[i - 4].y + data[i + 4].y + data[i - 3].y + data[i + 5].y + 5.0 * data[i].y) / 10.0,
            );
        }

        // Right border (j = 1..2, i = j * 4 + 3): (5 neighbors + 5 * self) / 10.0
        for j in 1..3 {
            let i = j * 4 + 3;
            buffer[i] = Pair::new(
                (data[i - 1].x + data[i - 4].x + data[i + 4].x + data[i - 5].x + data[i + 3].x + 5.0 * data[i].x) / 10.0,
                (data[i - 1].y + data[i - 4].y + data[i + 4].y + data[i - 5].y + data[i + 3].y + 5.0 * data[i].y) / 10.0,
            );
        }

        // Inner points: (8 neighbors + 8 * self) / 16.0
        for j in 1..3 {
            for i in 1..3 {
                let idx = i + j * 4;
                let sum_x = data[idx - 1].x + data[idx + 1].x + data[idx - 4].x + data[idx + 4].x
                    + data[idx - 5].x + data[idx - 3].x + data[idx + 3].x + data[idx + 5].x
                    + 8.0 * data[idx].x;
                let sum_y = data[idx - 1].y + data[idx + 1].y + data[idx - 4].y + data[idx + 4].y
                    + data[idx - 5].y + data[idx - 3].y + data[idx + 3].y + data[idx + 5].y
                    + 8.0 * data[idx].y;
                buffer[idx] = Pair::new(sum_x / 16.0, sum_y / 16.0);
            }
        }

        *data = *buffer;
    }

    /// Single sub-step of the physics simulation for elapsed time `time_ms` (in milliseconds),
    /// exactly matching `KWin::WobblyWindowsEffect::updateWindowWobblyDatas`.
    fn sub_step(&mut self, time_ms: f64, stiffness: f64, drag: f64, move_factor: f64) -> bool {
        let x_length = self.target_rect.size.w.max(1.0) / 3.0;
        let y_length = self.target_rect.size.h.max(1.0) / 3.0;

        // --- Compute accelerations for each vertex ---

        // 1. Corners
        // Top-left
        if self.constraint[0] {
            let mv = Pair::new(self.origin[0].x - self.position[0].x, self.origin[0].y - self.position[0].y);
            self.acceleration[0] = Pair::new(mv.x * stiffness, mv.y * stiffness);
        } else {
            let pos = self.position[0];
            let ax = ((self.position[1].x - pos.x) - x_length) * stiffness + (self.position[4].x - pos.x) * stiffness;
            let ay = ((self.position[4].y - pos.y) - y_length) * stiffness + (self.position[1].y - pos.y) * stiffness;
            self.acceleration[0] = Pair::new(ax * 0.5, ay * 0.5);
        }

        // Top-right
        if self.constraint[3] {
            let mv = Pair::new(self.origin[3].x - self.position[3].x, self.origin[3].y - self.position[3].y);
            self.acceleration[3] = Pair::new(mv.x * stiffness, mv.y * stiffness);
        } else {
            let pos = self.position[3];
            let ax = (x_length - (pos.x - self.position[2].x)) * stiffness + (self.position[7].x - pos.x) * stiffness;
            let ay = ((self.position[7].y - pos.y) - y_length) * stiffness + (self.position[2].y - pos.y) * stiffness;
            self.acceleration[3] = Pair::new(ax * 0.5, ay * 0.5);
        }

        // Bottom-left
        if self.constraint[12] {
            let mv = Pair::new(self.origin[12].x - self.position[12].x, self.origin[12].y - self.position[12].y);
            self.acceleration[12] = Pair::new(mv.x * stiffness, mv.y * stiffness);
        } else {
            let pos = self.position[12];
            let ax = ((self.position[13].x - pos.x) - x_length) * stiffness + (self.position[8].x - pos.x) * stiffness;
            let ay = (y_length - (pos.y - self.position[8].y)) * stiffness + (self.position[13].y - pos.y) * stiffness;
            self.acceleration[12] = Pair::new(ax * 0.5, ay * 0.5);
        }

        // Bottom-right
        if self.constraint[15] {
            let mv = Pair::new(self.origin[15].x - self.position[15].x, self.origin[15].y - self.position[15].y);
            self.acceleration[15] = Pair::new(mv.x * stiffness, mv.y * stiffness);
        } else {
            let pos = self.position[15];
            let ax = (x_length - (pos.x - self.position[14].x)) * stiffness + (self.position[11].x - pos.x) * stiffness;
            let ay = (y_length - (pos.y - self.position[11].y)) * stiffness + (self.position[14].y - pos.y) * stiffness;
            self.acceleration[15] = Pair::new(ax * 0.5, ay * 0.5);
        }

        // 2. Borders
        // Top border
        for i in 1..3 {
            if self.constraint[i] {
                let mv = Pair::new(self.origin[i].x - self.position[i].x, self.origin[i].y - self.position[i].y);
                self.acceleration[i] = Pair::new(mv.x * stiffness, mv.y * stiffness);
            } else {
                let pos = self.position[i];
                let ax = (x_length - (pos.x - self.position[i - 1].x)) * stiffness
                    + ((self.position[i + 1].x - pos.x) - x_length) * stiffness
                    + (self.position[i + 4].x - pos.x) * stiffness;
                let ay = ((self.position[i + 4].y - pos.y) - y_length) * stiffness
                    + (self.position[i - 1].y - pos.y) * stiffness
                    + (self.position[i + 1].y - pos.y) * stiffness;
                self.acceleration[i] = Pair::new(ax / 3.0, ay / 3.0);
            }
        }

        // Bottom border
        for i in 13..15 {
            if self.constraint[i] {
                let mv = Pair::new(self.origin[i].x - self.position[i].x, self.origin[i].y - self.position[i].y);
                self.acceleration[i] = Pair::new(mv.x * stiffness, mv.y * stiffness);
            } else {
                let pos = self.position[i];
                let ax = (x_length - (pos.x - self.position[i - 1].x)) * stiffness
                    + ((self.position[i + 1].x - pos.x) - x_length) * stiffness
                    + (self.position[i - 4].x - pos.x) * stiffness;
                let ay = (y_length - (pos.y - self.position[i - 4].y)) * stiffness
                    + (self.position[i - 1].y - pos.y) * stiffness
                    + (self.position[i + 1].y - pos.y) * stiffness;
                self.acceleration[i] = Pair::new(ax / 3.0, ay / 3.0);
            }
        }

        // Left border
        for j in 1..3 {
            let i = j * 4;
            if self.constraint[i] {
                let mv = Pair::new(self.origin[i].x - self.position[i].x, self.origin[i].y - self.position[i].y);
                self.acceleration[i] = Pair::new(mv.x * stiffness, mv.y * stiffness);
            } else {
                let pos = self.position[i];
                let ax = ((self.position[i + 1].x - pos.x) - x_length) * stiffness
                    + (self.position[i - 4].x - pos.x) * stiffness
                    + (self.position[i + 4].x - pos.x) * stiffness;
                let ay = (y_length - (pos.y - self.position[i - 4].y)) * stiffness
                    + ((self.position[i + 4].y - pos.y) - y_length) * stiffness
                    + (self.position[i + 1].y - pos.y) * stiffness;
                self.acceleration[i] = Pair::new(ax / 3.0, ay / 3.0);
            }
        }

        // Right border
        for j in 1..3 {
            let i = j * 4 + 3;
            if self.constraint[i] {
                let mv = Pair::new(self.origin[i].x - self.position[i].x, self.origin[i].y - self.position[i].y);
                self.acceleration[i] = Pair::new(mv.x * stiffness, mv.y * stiffness);
            } else {
                let pos = self.position[i];
                let ax = (x_length - (pos.x - self.position[i - 1].x)) * stiffness
                    + (self.position[i - 4].x - pos.x) * stiffness
                    + (self.position[i + 4].x - pos.x) * stiffness;
                let ay = (y_length - (pos.y - self.position[i - 4].y)) * stiffness
                    + ((self.position[i + 4].y - pos.y) - y_length) * stiffness
                    + (self.position[i - 1].y - pos.y) * stiffness;
                self.acceleration[i] = Pair::new(ax / 3.0, ay / 3.0);
            }
        }

        // 3. Inner points
        for j in 1..3 {
            for i in 1..3 {
                let idx = i + j * 4;
                if self.constraint[idx] {
                    let mv = Pair::new(self.origin[idx].x - self.position[idx].x, self.origin[idx].y - self.position[idx].y);
                    self.acceleration[idx] = Pair::new(mv.x * stiffness, mv.y * stiffness);
                } else {
                    let pos = self.position[idx];
                    let ax = ((self.position[idx - 1].x - pos.x) - x_length) * stiffness
                        + (x_length - (pos.x - self.position[idx + 1].x)) * stiffness
                        + (self.position[idx - 4].x - pos.x) * stiffness
                        + (self.position[idx + 4].x - pos.x) * stiffness;
                    let ay = (y_length - (pos.y - self.position[idx - 4].y)) * stiffness
                        + ((self.position[idx + 4].y - pos.y) - y_length) * stiffness
                        + (self.position[idx - 1].y - pos.y) * stiffness
                        + (self.position[idx + 1].y - pos.y) * stiffness;
                    self.acceleration[idx] = Pair::new(ax / 4.0, ay / 4.0);
                }
            }
        }

        // 1st smoothing: heightRingLinearMean on accelerations
        Self::height_ring_linear_mean(&mut self.acceleration, &mut self.buffer);

        // Integrate velocities
        let mut acc_sum = 0.0;
        for i in 0..16 {
            let mut acc = self.acceleration[i];
            fix_vector_bounds(&mut acc, Self::MIN_ACCELERATION, Self::MAX_ACCELERATION);

            let mut d_x = drag;
            let mut d_y = drag;
            if self.snapped_x {
                d_x *= 0.6;
            }
            if self.snapped_y {
                d_y *= 0.6;
            }

            self.velocity[i].x = acc.x * time_ms + self.velocity[i].x * d_x;
            self.velocity[i].y = acc.y * time_ms + self.velocity[i].y * d_y;
            acc_sum += acc.x.abs() + acc.y.abs();
        }

        // 2nd smoothing: heightRingLinearMean on velocities
        Self::height_ring_linear_mean(&mut self.velocity, &mut self.buffer);

        // Integrate positions
        let mut vel_sum = 0.0;
        for i in 0..16 {
            let mut vel = self.velocity[i];
            fix_vector_bounds(&mut vel, Self::MIN_VELOCITY, Self::MAX_VELOCITY);

            self.position[i].x += vel.x * time_ms * move_factor;
            self.position[i].y += vel.y * time_ms * move_factor;
            vel_sum += vel.x.abs() + vel.y.abs();
        }

        // Clamp magnetically snapped axes directly to origin to prevent boundary jitter/scaling artifacts
        if self.snapped_x {
            for i in 0..16 {
                self.position[i].x = self.origin[i].x;
                self.velocity[i].x = 0.0;
                self.acceleration[i].x = 0.0;
            }
        }
        if self.snapped_y {
            for i in 0..16 {
                self.position[i].y = self.origin[i].y;
                self.velocity[i].y = 0.0;
                self.acceleration[i].y = 0.0;
            }
        }

        let wobblying = !(acc_sum < Self::STOP_ACCELERATION && vel_sum < Self::STOP_VELOCITY);
        if !self.is_dragging && !wobblying {
            for i in 0..16 {
                self.position[i] = self.origin[i];
                self.velocity[i] = Pair::default();
            }
            self.settled = true;
            return false;
        }

        true
    }

    /// Advance the simulation by elapsed real time `dt_sec`.
    /// Employs 10ms sub-stepping matching KWin `integrationStep(10)`.
    pub fn step(&mut self, dt_sec: f64, stiffness: f64, drag: f64, move_factor: f64) -> bool {
        if self.settled {
            return false;
        }

        let mut delta_ms = (dt_sec * 1000.0).clamp(1.0, 50.0);
        let integration_step_ms = 10.0;

        let mut active = true;
        while delta_ms > 0.0 {
            let step_ms = delta_ms.min(integration_step_ms);
            delta_ms -= step_ms;
            if !self.sub_step(step_ms, stiffness, drag, move_factor) {
                active = false;
                break;
            }
        }

        active
    }

    /// Calculate visual location and size of the wobbly window using the 4x4 Bezier surface,
    /// matching KWin quad deformation bounding box.
    pub fn visual_rect(&self) -> Rectangle<f64, Logical> {
        let center = compute_bezier_point(&self.position, Pair::new(0.5, 0.5));
        let top = compute_bezier_point(&self.position, Pair::new(0.5, 0.0));
        let bottom = compute_bezier_point(&self.position, Pair::new(0.5, 1.0));
        let left = compute_bezier_point(&self.position, Pair::new(0.0, 0.5));
        let right = compute_bezier_point(&self.position, Pair::new(1.0, 0.5));

        let mut w = (right.x - left.x).abs().max(20.0);
        let mut h = (bottom.y - top.y).abs().max(20.0);
        let mut loc = Point::from((center.x - w / 2.0, center.y - h / 2.0));

        // When magnetically snapped, anchor the snapped axis directly to the snap target
        // and lock the size along that axis to target_rect so no scaling artifacts or gaps occur.
        if self.snapped_x {
            loc.x = self.target_rect.loc.x;
            w = self.target_rect.size.w;
        }
        if self.snapped_y {
            loc.y = self.target_rect.loc.y;
            h = self.target_rect.size.h;
        }

        Rectangle::new(loc, Size::from((w, h)))
    }

    /// Return horizontal and vertical shear angles based on Bezier edge offsets.
    #[allow(dead_code)]
    pub fn bezier_shear(&self) -> (f64, f64) {
        let top = compute_bezier_point(&self.position, Pair::new(0.5, 0.0));
        let bottom = compute_bezier_point(&self.position, Pair::new(0.5, 1.0));
        let left = compute_bezier_point(&self.position, Pair::new(0.0, 0.5));
        let right = compute_bezier_point(&self.position, Pair::new(1.0, 0.5));

        let h = self.target_rect.size.h.max(1.0);
        let w = self.target_rect.size.w.max(1.0);

        let shear_x = (top.x - bottom.x) / h;
        let shear_y = (left.y - right.y) / w;

        (shear_x, shear_y)
    }

    /// Normalized Bezier offsets relative to ideal target geometry for all 16 control points.
    #[allow(dead_code)]
    pub fn normalized_offsets(&self) -> [Pair; 16] {
        let mut offsets = [Pair::default(); 16];
        let w = self.target_rect.size.w.max(1.0);
        let h = self.target_rect.size.h.max(1.0);
        for i in 0..16 {
            offsets[i] = Pair::new(
                (self.position[i].x - self.origin[i].x) / w,
                (self.position[i].y - self.origin[i].y) / h,
            );
        }
        offsets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kwin_wobbly_initialization() {
        let rect = Rectangle::new(Point::from((100.0, 100.0)), Size::from((300.0, 300.0)));
        let grab = Point::from((150.0, 120.0));
        let wobbly = WobblyState::new(rect, grab);

        assert_eq!(wobbly.position.len(), 16);
        assert!(wobbly.constraint.iter().any(|&c| c));
        let vr = wobbly.visual_rect();
        assert!((vr.loc.x - rect.loc.x).abs() < 1.0);
        assert!((vr.loc.y - rect.loc.y).abs() < 1.0);
        assert!((vr.size.w - rect.size.w).abs() < 1.0);
        assert!((vr.size.h - rect.size.h).abs() < 1.0);
    }

    #[test]
    fn kwin_wobbly_drag_and_settle() {
        let rect1 = Rectangle::new(Point::from((100.0, 100.0)), Size::from((300.0, 300.0)));
        let grab1 = Point::from((150.0, 150.0));
        let mut wobbly = WobblyState::new(rect1, grab1);

        // Move window to new position
        let rect2 = Rectangle::new(Point::from((200.0, 200.0)), Size::from((300.0, 300.0)));
        let grab2 = Point::from((250.0, 250.0));
        wobbly.on_move(rect2, grab2, false, false);

        // Advance simulation for 5 frames while dragging
        for _ in 0..5 {
            wobbly.step(0.016, 0.15, 0.80, 0.10);
        }

        // Release the window
        wobbly.on_release();
        assert!(!wobbly.is_dragging);

        // Simulate until settled
        let mut settled = false;
        for _ in 0..300 {
            if !wobbly.step(0.016, 0.15, 0.80, 0.10) {
                settled = true;
                break;
            }
        }
        assert!(settled, "KWin wobbly window must settle after release");
        let vr = wobbly.visual_rect();
        assert!((vr.loc.x - rect2.loc.x).abs() < 0.5);
        assert!((vr.loc.y - rect2.loc.y).abs() < 0.5);
    }

    #[test]
    fn kwin_wobbly_magnetic_snapping_anchors_axis() {
        let rect1 = Rectangle::new(Point::from((100.0, 100.0)), Size::from((300.0, 300.0)));
        let grab1 = Point::from((150.0, 150.0));
        let mut wobbly = WobblyState::new(rect1, grab1);

        let rect2 = Rectangle::new(Point::from((500.0, 200.0)), Size::from((300.0, 300.0)));
        let grab2 = Point::from((570.0, 250.0));
        wobbly.on_move(rect2, grab2, true, false);

        for _ in 0..10 {
            wobbly.step(0.016, 0.15, 0.80, 0.10);
        }

        let vr = wobbly.visual_rect();
        assert_eq!(vr.loc.x, 500.0, "Snapped axis must remain anchored to target_rect.loc.x");
        assert_eq!(vr.size.w, 300.0, "Snapped axis width must not wobble or scale");
    }

    #[test]
    fn kwin_bezier_interpolation() {
        let mut pos = [Pair::default(); 16];
        for j in 0..4 {
            for i in 0..4 {
                pos[j * 4 + i] = Pair::new((i as f64) * 100.0, (j as f64) * 100.0);
            }
        }

        // Center must equal (150.0, 150.0)
        let center = compute_bezier_point(&pos, Pair::new(0.5, 0.5));
        assert!((center.x - 150.0).abs() < 1e-4);
        assert!((center.y - 150.0).abs() < 1e-4);

        // Top-left corner
        let tl = compute_bezier_point(&pos, Pair::new(0.0, 0.0));
        assert!((tl.x - 0.0).abs() < 1e-4);
        assert!((tl.y - 0.0).abs() < 1e-4);

        // Bottom-right corner
        let br = compute_bezier_point(&pos, Pair::new(1.0, 1.0));
        assert!((br.x - 300.0).abs() < 1e-4);
        assert!((br.y - 300.0).abs() < 1e-4);
    }
}
