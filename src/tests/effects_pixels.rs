//! Pixel-level test for the Fire and Glitch closing shaders.

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::texture::{TextureBuffer, TextureRenderElement};
use smithay::backend::renderer::gles::GlesTexture;
use smithay::backend::renderer::Offscreen;
use smithay::utils::{Physical, Point, Scale, Size, Transform};

use super::gl;
use crate::render::{
    DoomWindowElement, FireWindowElement, OutputRenderElements, TvWindowElement,
    WindowTransformElement, compile_doom_shader, compile_fire_shader, compile_tv_shader,
    render_elements_to_rgba,
};

#[test]
#[ignore = "needs Mesa surfaceless EGL; run with --include-ignored"]
fn fire_shader_renders_accurately() {
    let _gl = gl::lock();
    let Some(mut renderer) = gl::surfaceless_renderer("fire_shader_renders_accurately") else {
        return;
    };

    let Some(shader) = compile_fire_shader(&mut renderer) else {
        panic!("failed to compile fire shader");
    };

    let width = 200;
    let height = 200;
    let phys_size = Size::<i32, Physical>::from((width, height));
    let buffer_size = smithay::utils::Size::from((width, height));

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

    let fire = FireWindowElement::new(
        elem,
        shader,
        0.5,
        false,
        1.2,
        (width as f32, height as f32),
        1.0,
        0.5,
        (0.298, 0.200, 0.098, 0.0),
        (0.706, 0.216, 0.118, 0.7),
        (1.0, 0.298, 0.149, 0.9),
        (1.0, 0.651, 0.098, 1.0),
        (1.0, 1.0, 1.0, 1.0),
        false,
    );

    let center = Point::from((width as f64 / 2.0, height as f64 / 2.0));
    let transform_elem = OutputRenderElements::FireClosingWindow(
        WindowTransformElement::new(
            fire,
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
    .expect("render fire elements to rgba");

    assert_eq!(bytes.len(), (width * height * 4) as usize);

    let img = image::RgbaImage::from_raw(width as u32, height as u32, bytes)
        .expect("valid rgba image");
    let out_path = "/tmp/fire_driftwm_test.png";
    img.save(out_path).expect("save rendered fire image");

    // Pixels at the burn edge must have flame colors (warm hues, r > b)
    let sample_px = img.get_pixel(100, 100);
    assert!(sample_px[0] >= sample_px[2], "Fire should have warm red/orange hue");
}

#[test]
#[ignore = "needs Mesa surfaceless EGL; run with --include-ignored"]
fn tv_shader_renders_accurately() {
    let _gl = gl::lock();
    let Some(mut renderer) = gl::surfaceless_renderer("tv_shader_renders_accurately") else {
        return;
    };

    let Some(shader) = compile_tv_shader(&mut renderer) else {
        panic!("failed to compile tv shader");
    };

    let width = 200;
    let height = 200;
    let phys_size = Size::<i32, Physical>::from((width, height));
    let buffer_size = smithay::utils::Size::from((width, height));

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

    let tv = TvWindowElement::new(
        elem,
        shader,
        0.5,
        false,
        0.4,
        (width as f32, height as f32),
        (1.0, 1.0, 1.0, 1.0),
    );

    let center = Point::from((width as f64 / 2.0, height as f64 / 2.0));
    let transform_elem = OutputRenderElements::TvClosingWindow(
        WindowTransformElement::new(
            tv,
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
    .expect("render tv elements to rgba");

    assert_eq!(bytes.len(), (width * height * 4) as usize);

    let img = image::RgbaImage::from_raw(width as u32, height as u32, bytes)
        .expect("valid rgba image");
    let out_path = "/tmp/tv_driftwm_test.png";
    img.save(out_path).expect("save rendered tv image");
}

#[test]
#[ignore = "needs Mesa surfaceless EGL; run with --include-ignored"]
fn doom_shader_renders_accurately() {
    let _gl = gl::lock();
    let Some(mut renderer) = gl::surfaceless_renderer("doom_shader_renders_accurately") else {
        return;
    };

    let Some(shader) = compile_doom_shader(&mut renderer) else {
        panic!("failed to compile doom shader");
    };

    let width = 200;
    let height = 200;
    let phys_size = Size::<i32, Physical>::from((width, height));
    let buffer_size = smithay::utils::Size::from((width, height));

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

    let doom = DoomWindowElement::new(
        elem,
        shader,
        0.5,
        false,
        0.7,
        (width as f32, height as f32),
        2.0,
        3.0,
        4.0,
    );

    let center = Point::from((width as f64 / 2.0, height as f64 / 2.0));
    let transform_elem = OutputRenderElements::DoomClosingWindow(
        WindowTransformElement::new(
            doom,
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
    .expect("render doom elements to rgba");

    assert_eq!(bytes.len(), (width * height * 4) as usize);

    let img = image::RgbaImage::from_raw(width as u32, height as u32, bytes)
        .expect("valid rgba image");
    let out_path = "/tmp/doom_driftwm_test.png";
    img.save(out_path).expect("save rendered doom image");
}
