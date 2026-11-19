//! Eksport na GPU (ten sam shader co podgląd) musi dawać te same piksele co kompozycja CPU
//! z `asone-core` (używana przez CLI, MCP i wersję przeglądarkową).
//!
//! Wymaga karty graficznej: `cargo test -p asone --test gpu_export -- --ignored`.
#![cfg(not(target_arch = "wasm32"))]

use asone_app::renderer::{DocGpu, GpuShared, MipLevel, OffscreenSource, render_offscreen_tiled};
use asone_core::compose::compose_rgb;
use asone_core::edits::{EditParams, PxRect, fill_rect, stamp_circle};
use eframe::wgpu;
use image::{Rgba, RgbaImage};

fn gpu() -> Option<std::sync::Arc<GpuShared>> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
    eprintln!("GPU: {:?}", adapter.get_info().name);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
    Some(GpuShared::from_parts(device, queue, wgpu::TextureFormat::Rgba8Unorm))
}

#[test]
#[ignore = "wymaga GPU"]
fn eksport_gpu_zgodny_z_cpu() {
    let Some(shared) = gpu() else {
        panic!("brak adaptera wgpu");
    };
    let (w, h) = (257u32, 193u32);
    let img = RgbaImage::from_fn(w, h, |x, y| Rgba([(x * 255 / w) as u8, (y * 255 / h) as u8, ((x * 7 + y * 3) % 256) as u8, 255]));
    let mut mask = vec![255u8; (w * h) as usize];
    stamp_circle(&mut mask, [w, h], [100.0, 80.0], 30.0, false);
    fill_rect(&mut mask, [w, h], PxRect { x0: 200, y0: 10, x1: 240, y1: 60 }, 128);
    let gpu_doc = DocGpu::new(&shared, &[MipLevel { size: [w, h], data: img.as_raw().clone() }], &mask);
    let src = OffscreenSource {
        source_view: gpu_doc.source_view.clone(),
        mask_view: gpu_doc.mask_view.clone(),
        size: [w, h],
        mip_count: 1,
    };

    for (edits, tile) in [
        (EditParams::default(), 4096),
        (
            EditParams {
                crop: [0.1, 0.05, 0.9, 0.95],
                margins: [7, 11, 13, 5],
                bg_color: Some([0.2, 0.4, 0.6, 1.0]),
                use_bg_removal: true,
            },
            4096,
        ),
        // Wiele kafelków, w tym niepełne na krawędziach.
        (
            EditParams { margins: [3, 40, 0, 9], use_bg_removal: true, ..Default::default() },
            64,
        ),
    ] {
        let (size_gpu, rgb_gpu) = render_offscreen_tiled(&shared, &src, &edits, tile).unwrap();
        let (size_cpu, rgb_cpu) = compose_rgb(&img, Some(&mask), &edits);
        assert_eq!(size_gpu, size_cpu, "{edits:?}");
        let max_diff = rgb_gpu.iter().zip(&rgb_cpu).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
        eprintln!("{size_gpu:?} kafelek {tile}: maks. różnica {max_diff}");
        assert!(max_diff <= 1, "GPU i CPU różnią się o {max_diff} dla {edits:?}");
    }
}
