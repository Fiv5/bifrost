use tauri::window::Window;

#[cfg(target_os = "macos")]
mod imp {
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

        let cluster = NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), bounds);
        cluster.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        cluster.setMaterial(NSVisualEffectMaterial::UnderWindowBackground);
        cluster.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        cluster.setState(NSVisualEffectState::Active);
        cluster.setWantsLayer(true);

        if let Some(layer) = cluster.layer() {
            let fill = NSColor::colorWithSRGBRed_green_blue_alpha(0.94, 0.97, 1.0, 0.025);
            let border = NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, 0.14);
            let shadow = NSColor::colorWithSRGBRed_green_blue_alpha(0.05, 0.10, 0.16, 0.08);
            layer.setBackgroundColor(Some(&fill.CGColor()));
            layer.setCornerRadius(28.0);
            layer.setBorderWidth(0.6);
            layer.setBorderColor(Some(&border.CGColor()));
            layer.setShadowColor(Some(&shadow.CGColor()));
            layer.setShadowOpacity(0.24);
            layer.setShadowRadius(10.0);
            layer.setShadowOffset(NSSize::new(0.0, -1.0));
        }

        if let Some(glow_view) = create_glow_view(mtm) {
            cluster.addSubview(&glow_view);
        }

        if let Some(logo_view) = create_logo_view(mtm) {
            cluster.addSubview(&logo_view);
        }

        let title = NSTextField::labelWithString(&NSString::from_str("Bifrost"), mtm);
        title.setFrame(NSRect::new(
            NSPoint::new((bounds.size.width - 220.0) * 0.5, 56.0),
            NSSize::new(220.0, 30.0),
        ));
        title.setTextColor(Some(&NSColor::colorWithSRGBRed_green_blue_alpha(
            0.11, 0.16, 0.20, 0.92,
        )));
        title.setAlignment(NSTextAlignment::Center);
        title.setFont(Some(&NSFont::systemFontOfSize_weight(21.0, 0.42)));
        cluster.addSubview(&title);

        let subtitle = NSTextField::labelWithString(&NSString::from_str("Launching..."), mtm);
        subtitle.setFrame(NSRect::new(
            NSPoint::new((bounds.size.width - 236.0) * 0.5, 34.0),
            NSSize::new(236.0, 18.0),
        ));
        subtitle.setTextColor(Some(&NSColor::colorWithSRGBRed_green_blue_alpha(
            0.29, 0.37, 0.44, 0.78,
        )));
        subtitle.setAlignment(NSTextAlignment::Center);
        subtitle.setFont(Some(&NSFont::systemFontOfSize_weight(12.0, 0.28)));
        cluster.addSubview(&subtitle);

        overlay.addSubview(&cluster);
        content_view.addSubview(&overlay);
        Ok(Some(Retained::as_ptr(&overlay) as usize))
    }

    pub fn set_overlay_alpha(window: &Window, overlay_ptr: usize, alpha: f64) -> tauri::Result<()> {
        let _content_view = content_view(window)?;
        let overlay = unsafe { &*(overlay_ptr as *mut NSView) };
        overlay.setAlphaValue(alpha);
        Ok(())
    }

    pub fn remove_overlay(window: &Window, overlay_ptr: usize) -> tauri::Result<()> {
        let _content_view = content_view(window)?;
        let overlay = unsafe { &*(overlay_ptr as *mut NSView) };
        overlay.removeFromSuperview();
        Ok(())
    }

    fn create_logo_view(mtm: MainThreadMarker) -> Option<Retained<NSImageView>> {
        let logo_data = NSData::with_bytes(include_bytes!("../../../assets/bifrost.png"));
        let logo_image = NSImage::initWithData(NSImage::alloc(), &logo_data)?;
        let logo_view = NSImageView::imageViewWithImage(&logo_image, mtm);
        logo_view.setFrame(NSRect::new(
            NSPoint::new(140.0, 114.0),
            NSSize::new(80.0, 80.0),
        ));
        Some(logo_view)
    }

    fn create_glow_view(mtm: MainThreadMarker) -> Option<Retained<NSView>> {
        let glow = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(130.0, 104.0), NSSize::new(100.0, 100.0)),
        );
        glow.setWantsLayer(true);

        let layer: Retained<CALayer> = glow.layer()?;
        let ring_color = NSColor::colorWithSRGBRed_green_blue_alpha(0.84, 0.93, 1.0, 0.16);
        let shadow_color = NSColor::colorWithSRGBRed_green_blue_alpha(0.56, 0.88, 1.0, 0.24);

        layer.setBackgroundColor(Some(&NSColor::clearColor().CGColor()));
        layer.setCornerRadius(50.0);
        layer.setBorderWidth(1.5);
        layer.setBorderColor(Some(&ring_color.CGColor()));
        layer.setShadowColor(Some(&shadow_color.CGColor()));
        layer.setShadowOpacity(0.36);
        layer.setShadowRadius(12.0);
        layer.setShadowOffset(NSSize::new(0.0, 0.0));

        Some(glow)
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

    pub fn set_overlay_alpha(
        _window: &Window,
        _overlay_ptr: usize,
        _alpha: f64,
    ) -> tauri::Result<()> {
        Ok(())
    }

    pub fn remove_overlay(_window: &Window, _overlay_ptr: usize) -> tauri::Result<()> {
        Ok(())
    }
}

pub fn install(window: &Window) -> tauri::Result<Option<usize>> {
    imp::install(window)
}

pub fn set_overlay_alpha(window: &Window, overlay_ptr: usize, alpha: f64) -> tauri::Result<()> {
    imp::set_overlay_alpha(window, overlay_ptr, alpha)
}

pub fn remove_overlay(window: &Window, overlay_ptr: usize) -> tauri::Result<()> {
    imp::remove_overlay(window, overlay_ptr)
}
