//! Pixel-level test for the Rick and Morty Portal closing shader.

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::texture::TextureRenderElement;
use smithay::backend::renderer::gles::GlesTexture;
use smithay::backend::renderer::Offscreen;
use smithay::backend::renderer::element::texture::TextureBuffer;
use smithay::utils::{Physical, Point, Scale, Size, Transform};

use super::gl;
use crate::render::{
    OutputRenderElements, PortalWindowElement, WindowTransformElement,
    compile_portal_shader, render_elements_to_rgba,
};

#[test]
#[ignore = "needs Mesa surfaceless EGL; run with --include-ignored"]
fn portal_shader_renders_accurately() {
    let _gl = gl::lock();
    let Some(mut renderer) = gl::surfaceless_renderer("portal_shader_renders_accurately") else {
        return;
    };

    let Some(shader) = compile_portal_shader(&mut renderer) else {
        panic!("failed to compile portal shader");
    };

    let width = 200;
    let height = 200;
    let phys_size = Size::<i32, Physical>::from((width, height));
    let buffer_size = smithay::utils::Size::from((width, height));

    // Allocate an offscreen dummy window texture (white window)
    let texture = Offscreen::<GlesTexture>::create_buffer(
        &mut renderer,
        Fourcc::Abgr8888,
        buffer_size,
    )
    .expect("create offscreen texture");

    let tex_buf = TextureBuffer::from_texture(&mut renderer, texture, 1, Transform::Normal, None);
    let elem = TextureRenderElement::from_texture_buffer(
        Point::<f64, Physical>::from((0.0, 0.0)),
        &tex_buf,
        Some(1.0),
        None,
        Some(smithay::utils::Size::from((width, height))),
        Kind::Unspecified,
    );

    // Setup portal element with testMode uniforms (progress = 0.5, seed = (0.0, 0.0))
    let portal = PortalWindowElement::new(
        elem,
        shader,
        0.5,
        false,
        8.0,
        (width as f32, height as f32),
        (0.4745, 0.9137, 0.0941),
        (0.0, 0.0),
        0.3,
        1.0,
        15.0,
    );

    let center = Point::from((width as f64 / 2.0, height as f64 / 2.0));
    let transform_elem = OutputRenderElements::PortalClosingWindow(
        WindowTransformElement::new(
            portal,
            center,
            Point::default(),
            Scale::from(1.0),
        ),
    );

    let refs = [&transform_elem];
    let bytes = render_elements_to_rgba(
        &mut renderer,
        phys_size,
        Scale::from(1.0),
        &refs,
    )
    .expect("render portal elements to rgba");

    assert_eq!(bytes.len(), (width * height * 4) as usize);

    // Save image to /tmp for visual inspection & comparison
    let img = image::RgbaImage::from_raw(width as u32, height as u32, bytes)
        .expect("valid rgba image");
    let out_path = "/tmp/portal_driftwm_test.png";
    img.save(out_path).expect("save rendered portal image");

    // Center pixel of the portal (100, 100) must be dark green / black singularity
    let center_px = img.get_pixel(100, 100);
    // Outer border (around radius 40-70) must have vibrant green
    let mid_px = img.get_pixel(130, 100);

    // Corner pixel (0, 0) must be fully transparent
    let corner_px = img.get_pixel(0, 0);

    assert_eq!(corner_px[3], 0, "corner outside portal must be transparent");
    assert!(mid_px[1] > 50, "plasma ring must have green channel > 50, got: {:?}", mid_px);
    assert!(center_px[3] > 0, "center of portal must not be transparent, got: {:?}", center_px);
}
