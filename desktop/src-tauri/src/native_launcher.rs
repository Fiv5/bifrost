use tauri::window::Window;

#[cfg(target_os = "macos")]
mod imp {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    use objc2::rc::Retained;
    use objc2::{AnyThread, MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSAutoresizingMaskOptions, NSColor, NSFont, NSImage, NSImageView, NSTextAlignment,
        NSTextField, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
        NSVisualEffectState, NSVisualEffectView,
    };
    use objc2_foundation::{NSData, NSPoint, NSRect, NSSize, NSString};
    use objc2_quartz_core::CALayer;
    use tauri::window::Window;

    const OVERLAY_BASE_ALPHA: f64 = 0.86;
    const CIRCLE_DIAMETER: f64 = 196.0;
    const FINAL_INSET_X: f64 = 18.0;
    const FINAL_INSET_Y: f64 = 18.0;
    const FINAL_CORNER_RADIUS: f64 = 30.0;
    const GLOW_EXPAND_PADDING: f64 = 24.0;
    const SEGMENT_COUNT: usize = 18;
    const SEGMENT_WIDTH: f64 = 10.0;
    const SEGMENT_HEIGHT: f64 = 2.5;
    const RING_PADDING: f64 = 8.0;
    const TITLE_MAX_ALPHA: f64 = 0.94;
    const SUBTITLE_MAX_ALPHA: f64 = 0.78;
    const ANIMATION_FRAME_DELAY: Duration = Duration::from_millis(54);

    struct LauncherOverlayHandle {
        overlay: Retained<NSView>,
        cluster: Retained<NSVisualEffectView>,
        glow: Retained<NSView>,
        edge_aura: Retained<NSView>,
        reflection: Retained<NSView>,
        logo: Retained<NSImageView>,
        title: Retained<NSTextField>,
        subtitle: Retained<NSTextField>,
        ring_segments: Vec<Retained<NSView>>,
        current_progress: Mutex<f64>,
        animation_running: Arc<AtomicBool>,
        animation_thread: Mutex<Option<JoinHandle<()>>>,
    }

    struct OverlayLayout {
        cluster_frame: NSRect,
        glow_frame: NSRect,
        corner_radius: f64,
        border_width: f64,
        title_frame: NSRect,
        subtitle_frame: NSRect,
        logo_frame: NSRect,
        title_alpha: f64,
        subtitle_alpha: f64,
        glow_alpha: f64,
        cluster_fill_alpha: f64,
    }

    pub fn install(window: &Window) -> tauri::Result<Option<usize>> {
        let mtm = MainThreadMarker::new().ok_or_else(|| {
            crate::anyhow("native launcher must be installed on the main thread".to_string())
        })?;
        let content_view = content_view(window)?;
        let bounds = content_view.bounds();
        let overlay = NSView::initWithFrame(NSView::alloc(mtm), bounds);
        overlay.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        overlay.setAlphaValue(1.0);
        overlay.setWantsLayer(true);
        if let Some(layer) = overlay.layer() {
            layer.setBackgroundColor(Some(&NSColor::clearColor().CGColor()));
        }

        let cluster = NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), bounds);
        cluster.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        cluster.setMaterial(NSVisualEffectMaterial::UnderWindowBackground);
        cluster.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        cluster.setState(NSVisualEffectState::Active);
        cluster.setWantsLayer(true);

        let glow_view = create_glow_view(mtm)?;
        cluster.addSubview(&glow_view);

        let edge_aura_view = create_edge_aura_view(mtm)?;
        cluster.addSubview(&edge_aura_view);

        let reflection_view = create_reflection_view(mtm)?;
        cluster.addSubview(&reflection_view);

        let ring_segments = create_ring_segments(mtm);
        for segment in &ring_segments {
            cluster.addSubview(segment);
        }

        let logo_view = create_logo_view(mtm)?;
        cluster.addSubview(&logo_view);

        let title = NSTextField::labelWithString(&NSString::from_str("Bifrost"), mtm);
        title.setTextColor(Some(&NSColor::colorWithSRGBRed_green_blue_alpha(
            0.10, 0.18, 0.28, 0.96,
        )));
        title.setAlignment(NSTextAlignment::Center);
        title.setFont(Some(&NSFont::systemFontOfSize_weight(22.0, 0.44)));
        cluster.addSubview(&title);

        let subtitle = NSTextField::labelWithString(&NSString::from_str("Launching..."), mtm);
        subtitle.setTextColor(Some(&NSColor::colorWithSRGBRed_green_blue_alpha(
            0.20, 0.42, 0.64, 0.90,
        )));
        subtitle.setAlignment(NSTextAlignment::Center);
        subtitle.setFont(Some(&NSFont::systemFontOfSize_weight(12.0, 0.28)));
        cluster.addSubview(&subtitle);

        overlay.addSubview(&cluster);
        content_view.addSubview(&overlay);

        let handle = Box::new(LauncherOverlayHandle {
            overlay,
            cluster,
            glow: glow_view,
            edge_aura: edge_aura_view,
            reflection: reflection_view,
            logo: logo_view,
            title,
            subtitle,
            ring_segments,
            current_progress: Mutex::new(0.0),
            animation_running: Arc::new(AtomicBool::new(true)),
            animation_thread: Mutex::new(None),
        });

        apply_progress(handle.as_ref(), 0.0);
        apply_ring_tick(handle.as_ref(), 0);

        Ok(Some(Box::into_raw(handle) as usize))
    }

    pub fn start_animation(window: &Window, overlay_ptr: usize) -> tauri::Result<()> {
        let handle = unsafe { &mut *(overlay_ptr as *mut LauncherOverlayHandle) };
        let Ok(mut animation_thread) = handle.animation_thread.lock() else {
            return Ok(());
        };
        if animation_thread.is_some() {
            return Ok(());
        }

        let window = window.clone();
        let running = handle.animation_running.clone();
        *animation_thread = Some(thread::spawn(move || {
            let mut tick = 0_u64;
            while running.load(Ordering::Relaxed) {
                let window_for_tick = window.clone();
                let _ = window.run_on_main_thread(move || {
                    let _ = tick_overlay(&window_for_tick, overlay_ptr, tick);
                });
                thread::sleep(ANIMATION_FRAME_DELAY);
                tick = tick.wrapping_add(1);
            }
        }));

        Ok(())
    }

    pub fn set_overlay_alpha(window: &Window, overlay_ptr: usize, alpha: f64) -> tauri::Result<()> {
        let _content_view = content_view(window)?;
        let handle = unsafe { &*(overlay_ptr as *mut LauncherOverlayHandle) };
        handle
            .overlay
            .setAlphaValue(alpha.clamp(0.0, 1.0) * OVERLAY_BASE_ALPHA);
        Ok(())
    }

    pub fn set_overlay_progress(
        window: &Window,
        overlay_ptr: usize,
        progress: f64,
    ) -> tauri::Result<()> {
        let _content_view = content_view(window)?;
        let handle = unsafe { &*(overlay_ptr as *mut LauncherOverlayHandle) };
        apply_progress(handle, progress);
        Ok(())
    }

    pub fn tick_overlay(window: &Window, overlay_ptr: usize, tick: u64) -> tauri::Result<()> {
        let _content_view = content_view(window)?;
        let handle = unsafe { &*(overlay_ptr as *mut LauncherOverlayHandle) };
        apply_ring_tick(handle, tick);
        Ok(())
    }

    pub fn remove_overlay(window: &Window, overlay_ptr: usize) -> tauri::Result<()> {
        let _content_view = content_view(window)?;
        let handle = unsafe { Box::from_raw(overlay_ptr as *mut LauncherOverlayHandle) };
        handle.animation_running.store(false, Ordering::Relaxed);
        if let Ok(mut animation_thread) = handle.animation_thread.lock() {
            if let Some(join_handle) = animation_thread.take() {
                let _ = join_handle.join();
            }
        }
        handle.overlay.removeFromSuperview();
        Ok(())
    }

    fn create_logo_view(mtm: MainThreadMarker) -> tauri::Result<Retained<NSImageView>> {
        let logo_data = NSData::with_bytes(include_bytes!("../../../assets/bifrost.png"));
        let logo_image = NSImage::initWithData(NSImage::alloc(), &logo_data)
            .ok_or_else(|| crate::anyhow("failed to load launcher logo image".to_string()))?;
        let logo_view = NSImageView::imageViewWithImage(&logo_image, mtm);
        Ok(logo_view)
    }

    fn create_glow_view(mtm: MainThreadMarker) -> tauri::Result<Retained<NSView>> {
        let glow = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(10.0, 10.0)),
        );
        glow.setWantsLayer(true);

        let layer: Retained<CALayer> = glow
            .layer()
            .ok_or_else(|| crate::anyhow("failed to create launcher glow layer".to_string()))?;
        let fill_color = NSColor::colorWithSRGBRed_green_blue_alpha(0.98, 1.0, 1.0, 0.04);
        let ring_color = NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, 0.20);
        let shadow_color = NSColor::colorWithSRGBRed_green_blue_alpha(0.80, 0.92, 1.0, 0.16);

        layer.setBackgroundColor(Some(&fill_color.CGColor()));
        layer.setBorderWidth(0.8);
        layer.setBorderColor(Some(&ring_color.CGColor()));
        layer.setShadowColor(Some(&shadow_color.CGColor()));
        layer.setShadowOpacity(0.44);
        layer.setShadowRadius(52.0);
        layer.setShadowOffset(NSSize::new(0.0, 0.0));

        Ok(glow)
    }

    fn create_edge_aura_view(mtm: MainThreadMarker) -> tauri::Result<Retained<NSView>> {
        let edge_aura = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(10.0, 10.0)),
        );
        edge_aura.setWantsLayer(true);

        let layer: Retained<CALayer> = edge_aura.layer().ok_or_else(|| {
            crate::anyhow("failed to create launcher edge aura layer".to_string())
        })?;
        let border_color = NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, 0.28);
        let shadow_color = NSColor::colorWithSRGBRed_green_blue_alpha(0.90, 0.97, 1.0, 0.18);
        layer.setBackgroundColor(Some(&NSColor::clearColor().CGColor()));
        layer.setBorderWidth(1.0);
        layer.setBorderColor(Some(&border_color.CGColor()));
        layer.setShadowColor(Some(&shadow_color.CGColor()));
        layer.setShadowOpacity(0.28);
        layer.setShadowRadius(24.0);
        layer.setShadowOffset(NSSize::new(0.0, 0.0));

        Ok(edge_aura)
    }

    fn create_reflection_view(mtm: MainThreadMarker) -> tauri::Result<Retained<NSView>> {
        let reflection = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(10.0, 10.0)),
        );
        reflection.setWantsLayer(true);

        let layer: Retained<CALayer> = reflection.layer().ok_or_else(|| {
            crate::anyhow("failed to create launcher reflection layer".to_string())
        })?;
        let fill = NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, 0.16);
        let glow = NSColor::colorWithSRGBRed_green_blue_alpha(0.94, 0.98, 1.0, 0.18);
        layer.setBackgroundColor(Some(&fill.CGColor()));
        layer.setShadowColor(Some(&glow.CGColor()));
        layer.setShadowOpacity(0.20);
        layer.setShadowRadius(22.0);
        layer.setShadowOffset(NSSize::new(0.0, 0.0));

        Ok(reflection)
    }

    fn create_ring_segments(mtm: MainThreadMarker) -> Vec<Retained<NSView>> {
        (0..SEGMENT_COUNT)
            .map(|_| {
                let segment = NSView::initWithFrame(
                    NSView::alloc(mtm),
                    NSRect::new(
                        NSPoint::new(0.0, 0.0),
                        NSSize::new(SEGMENT_WIDTH, SEGMENT_HEIGHT),
                    ),
                );
                segment.setWantsLayer(true);
                if let Some(layer) = segment.layer() {
                    let color = NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, 0.92);
                    let glow = NSColor::colorWithSRGBRed_green_blue_alpha(0.94, 0.98, 1.0, 0.28);
                    layer.setBackgroundColor(Some(&color.CGColor()));
                    layer.setCornerRadius(SEGMENT_HEIGHT * 0.5);
                    layer.setShadowColor(Some(&glow.CGColor()));
                    layer.setShadowOpacity(0.18);
                    layer.setShadowRadius(5.0);
                    layer.setShadowOffset(NSSize::new(0.0, 0.0));
                }
                segment
            })
            .collect()
    }

    fn apply_progress(handle: &LauncherOverlayHandle, progress: f64) {
        let progress = progress.clamp(0.0, 1.0);
        if let Ok(mut current_progress) = handle.current_progress.lock() {
            *current_progress = progress;
        }
        let layout = overlay_layout(handle.overlay.bounds(), progress);
        handle.cluster.setFrame(layout.cluster_frame);
        handle.glow.setFrame(layout.glow_frame);
        handle.edge_aura.setFrame(NSRect::new(
            NSPoint::new(-4.0, -4.0),
            NSSize::new(
                layout.cluster_frame.size.width + 8.0,
                layout.cluster_frame.size.height + 8.0,
            ),
        ));
        let reflection_width = layout.cluster_frame.size.width * 0.40;
        let reflection_height = layout.cluster_frame.size.height * 0.20;
        handle.reflection.setFrame(NSRect::new(
            NSPoint::new(
                layout.cluster_frame.size.width * 0.16,
                layout.cluster_frame.size.height * 0.72,
            ),
            NSSize::new(reflection_width, reflection_height),
        ));
        handle.logo.setFrame(layout.logo_frame);
        handle.title.setFrame(layout.title_frame);
        handle.subtitle.setFrame(layout.subtitle_frame);
        handle.title.setAlphaValue(layout.title_alpha);
        handle.subtitle.setAlphaValue(layout.subtitle_alpha);
        handle.glow.setAlphaValue(layout.glow_alpha);
        handle
            .edge_aura
            .setAlphaValue(0.56 * (1.0 - progress * 0.66));
        handle
            .reflection
            .setAlphaValue((0.22 - progress * 0.08).clamp(0.0, 0.22));

        if let Some(layer) = handle.cluster.layer() {
            let fill = NSColor::colorWithSRGBRed_green_blue_alpha(
                0.07,
                0.11,
                0.18,
                layout.cluster_fill_alpha,
            );
            let border = NSColor::colorWithSRGBRed_green_blue_alpha(
                1.0,
                1.0,
                1.0,
                lerp(0.30, 0.12, progress),
            );
            let shadow = NSColor::colorWithSRGBRed_green_blue_alpha(
                0.78,
                0.92,
                1.0,
                lerp(0.22, 0.10, progress),
            );
            layer.setBackgroundColor(Some(&fill.CGColor()));
            layer.setCornerRadius(layout.corner_radius);
            layer.setBorderWidth(layout.border_width);
            layer.setBorderColor(Some(&border.CGColor()));
            layer.setShadowColor(Some(&shadow.CGColor()));
            layer.setShadowOpacity((0.18 + progress * 0.10) as f32);
            layer.setShadowRadius(18.0 + progress * 10.0);
            layer.setShadowOffset(NSSize::new(0.0, 0.0));
        }

        if let Some(layer) = handle.glow.layer() {
            layer.setCornerRadius(layout.glow_frame.size.width * 0.5);
        }
        if let Some(layer) = handle.edge_aura.layer() {
            layer.setCornerRadius((layout.cluster_frame.size.width + 8.0) * 0.5);
        }
        if let Some(layer) = handle.reflection.layer() {
            layer.setCornerRadius(layout.cluster_frame.size.height * 0.10);
        }

        layout_ring_segments(handle, progress, 0);
    }

    fn apply_ring_tick(handle: &LauncherOverlayHandle, tick: u64) {
        let progress = handle
            .current_progress
            .lock()
            .map(|current_progress| *current_progress)
            .unwrap_or(0.0);
        layout_ring_segments(handle, progress, tick);
    }

    fn layout_ring_segments(handle: &LauncherOverlayHandle, progress: f64, tick: u64) {
        let bounds = handle.cluster.bounds();
        let width = bounds.size.width.max(1.0);
        let height = bounds.size.height.max(1.0);
        let radius_x = (width * 0.5 + RING_PADDING).max(24.0);
        let radius_y = (height * 0.5 + RING_PADDING).max(24.0);
        let center_x = bounds.size.width * 0.5;
        let center_y = bounds.size.height * 0.5;
        let fade = 1.0 - progress.clamp(0.0, 1.0) * 0.84;
        let pulse_wave = ((tick as f64 / 9.0).sin() + 1.0) * 0.5;
        let sweep_center = (tick as f64 * 0.12) % SEGMENT_COUNT as f64;
        let arc_start = -2.55_f64;
        let arc_end = -0.62_f64;
        let arc_span = arc_end - arc_start;

        for (index, segment) in handle.ring_segments.iter().enumerate() {
            let t = index as f64 / (SEGMENT_COUNT.saturating_sub(1)) as f64;
            let angle = arc_start + arc_span * t;
            let x = center_x + radius_x * angle.cos() - SEGMENT_WIDTH * 0.5;
            let y = center_y + radius_y * angle.sin() - SEGMENT_HEIGHT * 0.5;
            segment.setFrame(NSRect::new(
                NSPoint::new(x, y),
                NSSize::new(SEGMENT_WIDTH, SEGMENT_HEIGHT),
            ));

            let distance_to_sweep = (index as f64 - sweep_center).abs();
            let sweep = (1.0 - (distance_to_sweep / 4.6)).clamp(0.0, 1.0);
            let arc_bias = (1.0 - (t - 0.22).abs() / 0.78).clamp(0.0, 1.0).powf(1.8);
            let shimmer = ((tick as f64 / 12.0) + index as f64 * 0.12).sin() * 0.5 + 0.5;
            let luminance = (0.05 + arc_bias * 0.16 + sweep * 0.42 + shimmer * 0.03) * fade;

            segment.setAlphaValue(luminance);
            if let Some(layer) = segment.layer() {
                layer.setShadowOpacity((0.06 + arc_bias * 0.10 + sweep * 0.18) as f32);
                layer.setShadowRadius(4.0 + arc_bias * 2.0 + sweep * 5.0);
                let corner_radius = SEGMENT_HEIGHT * 0.5 + sweep * 0.8;
                layer.setCornerRadius(corner_radius);
            }
        }

        if let Some(layer) = handle.glow.layer() {
            handle.glow.setAlphaValue(0.18 + pulse_wave * 0.07);
            layer.setShadowOpacity((0.14 + pulse_wave * 0.06) as f32);
            layer.setShadowRadius(52.0 + pulse_wave * 10.0);
        }

        if let Some(layer) = handle.edge_aura.layer() {
            let border =
                NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, 0.14 + pulse_wave * 0.06);
            let shadow = NSColor::colorWithSRGBRed_green_blue_alpha(
                0.92,
                0.98,
                1.0,
                0.12 + pulse_wave * 0.05,
            );
            layer.setBorderColor(Some(&border.CGColor()));
            layer.setShadowColor(Some(&shadow.CGColor()));
            layer.setShadowOpacity((0.10 + pulse_wave * 0.05) as f32);
            layer.setShadowRadius(22.0 + pulse_wave * 6.0);
        }

        if let Some(layer) = handle.reflection.layer() {
            let sweep = ((tick as f64 / 18.0).sin() + 1.0) * 0.5;
            let fill =
                NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, 0.10 + sweep * 0.08);
            let shadow =
                NSColor::colorWithSRGBRed_green_blue_alpha(0.94, 0.98, 1.0, 0.10 + sweep * 0.06);
            layer.setBackgroundColor(Some(&fill.CGColor()));
            layer.setShadowColor(Some(&shadow.CGColor()));
            layer.setShadowOpacity((0.08 + sweep * 0.06) as f32);
            layer.setShadowRadius(18.0 + sweep * 8.0);
        }
    }

    fn overlay_layout(bounds: NSRect, progress: f64) -> OverlayLayout {
        let start_frame = NSRect::new(
            NSPoint::new(
                (bounds.size.width - CIRCLE_DIAMETER) * 0.5,
                (bounds.size.height - CIRCLE_DIAMETER) * 0.5 + 6.0,
            ),
            NSSize::new(CIRCLE_DIAMETER, CIRCLE_DIAMETER),
        );
        let end_frame = NSRect::new(
            NSPoint::new(FINAL_INSET_X, FINAL_INSET_Y),
            NSSize::new(
                (bounds.size.width - FINAL_INSET_X * 2.0).max(CIRCLE_DIAMETER),
                (bounds.size.height - FINAL_INSET_Y * 2.0).max(CIRCLE_DIAMETER),
            ),
        );
        let cluster_frame = rect_lerp(start_frame, end_frame, progress);
        let glow_padding = lerp(GLOW_EXPAND_PADDING, 10.0, progress).max(0.0);
        let local_width = cluster_frame.size.width.max(1.0);
        let local_height = cluster_frame.size.height.max(1.0);
        let glow_frame = NSRect::new(
            NSPoint::new(-glow_padding, -glow_padding),
            NSSize::new(
                local_width + glow_padding * 2.0,
                local_height + glow_padding * 2.0,
            ),
        );
        let title_width = cluster_frame.size.width.min(260.0);
        let subtitle_width = cluster_frame.size.width.min(240.0);
        let center_x = local_width * 0.5;
        let title_y = local_height * 0.18;
        let subtitle_y = title_y - 22.0;
        let logo_size = lerp(74.0, 88.0, progress);
        let logo_y = local_height * 0.52 - logo_size * 0.5;

        OverlayLayout {
            cluster_frame,
            glow_frame,
            corner_radius: lerp(CIRCLE_DIAMETER * 0.5, FINAL_CORNER_RADIUS, progress),
            border_width: lerp(1.4, 1.0, progress),
            title_frame: NSRect::new(
                NSPoint::new(center_x - title_width * 0.5, title_y),
                NSSize::new(title_width, 30.0),
            ),
            subtitle_frame: NSRect::new(
                NSPoint::new(center_x - subtitle_width * 0.5, subtitle_y),
                NSSize::new(subtitle_width, 18.0),
            ),
            logo_frame: NSRect::new(
                NSPoint::new(center_x - logo_size * 0.5, logo_y),
                NSSize::new(logo_size, logo_size),
            ),
            title_alpha: TITLE_MAX_ALPHA * (1.0 - progress * 0.34),
            subtitle_alpha: SUBTITLE_MAX_ALPHA * (1.0 - progress * 0.42),
            glow_alpha: 0.54 + (1.0 - progress) * 0.12,
            cluster_fill_alpha: lerp(0.20, 0.34, progress),
        }
    }

    fn rect_lerp(start: NSRect, end: NSRect, progress: f64) -> NSRect {
        NSRect::new(
            NSPoint::new(
                lerp(start.origin.x, end.origin.x, progress),
                lerp(start.origin.y, end.origin.y, progress),
            ),
            NSSize::new(
                lerp(start.size.width, end.size.width, progress),
                lerp(start.size.height, end.size.height, progress),
            ),
        )
    }

    fn lerp(start: f64, end: f64, progress: f64) -> f64 {
        start + (end - start) * progress
    }

    fn content_view(window: &Window) -> tauri::Result<&NSView> {
        let ns_view = window.ns_view()?;
        let Some(content_view) = (unsafe { (ns_view as *mut NSView).as_ref() }) else {
            return Err(crate::anyhow(
                "failed to access macOS content view".to_string(),
            ));
        };
        Ok(content_view)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use tauri::window::Window;

    pub fn install(_window: &Window) -> tauri::Result<Option<usize>> {
        Ok(None)
    }

    pub fn start_animation(_window: &Window, _overlay_ptr: usize) -> tauri::Result<()> {
        Ok(())
    }

    pub fn set_overlay_alpha(
        _window: &Window,
        _overlay_ptr: usize,
        _alpha: f64,
    ) -> tauri::Result<()> {
        Ok(())
    }

    pub fn set_overlay_progress(
        _window: &Window,
        _overlay_ptr: usize,
        _progress: f64,
    ) -> tauri::Result<()> {
        Ok(())
    }

    pub fn tick_overlay(_window: &Window, _overlay_ptr: usize, _tick: u64) -> tauri::Result<()> {
        Ok(())
    }

    pub fn remove_overlay(_window: &Window, _overlay_ptr: usize) -> tauri::Result<()> {
        Ok(())
    }
}

pub fn install(window: &Window) -> tauri::Result<Option<usize>> {
    imp::install(window)
}

pub fn start_animation(window: &Window, overlay_ptr: usize) -> tauri::Result<()> {
    imp::start_animation(window, overlay_ptr)
}

pub fn set_overlay_alpha(window: &Window, overlay_ptr: usize, alpha: f64) -> tauri::Result<()> {
    imp::set_overlay_alpha(window, overlay_ptr, alpha)
}

pub fn set_overlay_progress(
    window: &Window,
    overlay_ptr: usize,
    progress: f64,
) -> tauri::Result<()> {
    imp::set_overlay_progress(window, overlay_ptr, progress)
}

pub fn remove_overlay(window: &Window, overlay_ptr: usize) -> tauri::Result<()> {
    imp::remove_overlay(window, overlay_ptr)
}
