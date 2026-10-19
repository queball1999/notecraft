//! Choosing the wgpu backend at startup.
//!
//! On Linux X11 with no GPU driver (a Flatpak without the matching GL extension, or
//! `--nodevice=dri`), the only Vulkan adapter is Mesa's software renderer (llvmpipe). Its X11
//! surface code leaves an error behind that winit 0.30 turns into a panic at the first
//! `XMapRaised` (docs/gaps.md). So on Linux we look at the Vulkan adapters before eframe starts,
//! and when there is no real GPU among them we start eframe on the GL backend instead.
//!
//! The decision is a pure function ([`choose_backends`]); the probe is thin, Linux only, and any
//! failure of it leaves eframe's default behavior alone. `WGPU_BACKEND` always wins.

use eframe::egui_wgpu::wgpu::{Backend, Backends, DeviceType};

/// Picks the backends eframe may use, or `None` to keep eframe's default (Vulkan first, GL
/// as a fallback; both honoring `WGPU_BACKEND`).
///
/// - A non-empty `WGPU_BACKEND` always wins: `None`.
/// - Any real Vulkan adapter (anything but `DeviceType::Cpu`): `None`.
/// - Only software Vulkan adapters, or none at all: GL only.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn choose_backends(
    adapters: &[(Backend, DeviceType)],
    wgpu_backend_env: Option<&str>,
) -> Option<Backends> {
    if wgpu_backend_env.is_some_and(|v| !v.trim().is_empty()) {
        return None;
    }
    let has_real_gpu = adapters
        .iter()
        .any(|&(backend, kind)| backend == Backend::Vulkan && kind != DeviceType::Cpu);
    if has_real_gpu {
        None
    } else {
        Some(Backends::GL)
    }
}

/// Applies the startup choice to the eframe options (Linux only; a no-op elsewhere).
pub fn configure(options: &mut eframe::NativeOptions) {
    #[cfg(target_os = "linux")]
    linux::configure(options);
    #[cfg(not(target_os = "linux"))]
    let _ = options;
}

#[cfg(target_os = "linux")]
mod linux {
    use super::choose_backends;
    use eframe::egui_wgpu::wgpu::{Backend, Backends, DeviceType, Instance, InstanceDescriptor};
    use eframe::egui_wgpu::WgpuSetup;

    pub fn configure(options: &mut eframe::NativeOptions) {
        let env = std::env::var("WGPU_BACKEND").ok();
        let Some(adapters) = probe_vulkan() else {
            tracing::info!("Vulkan probe failed; leaving the graphics backend choice to eframe");
            return;
        };
        let Some(backends) = choose_backends(&adapters, env.as_deref()) else {
            return;
        };
        let WgpuSetup::CreateNew(create_new) = &mut options.wgpu_options.wgpu_setup else {
            return;
        };
        tracing::info!(
            "no GPU-backed Vulkan adapter (only software or none); using the GL backend. \
             Set WGPU_BACKEND=vulkan to force Vulkan"
        );
        create_new.instance_descriptor.backends = backends;
    }

    /// Lists the Vulkan adapters (no surface needed); `None` when the probe could not finish.
    fn probe_vulkan() -> Option<Vec<(Backend, DeviceType)>> {
        let mut desc = InstanceDescriptor::new_without_display_handle();
        desc.backends = Backends::VULKAN;
        let instance = Instance::new(desc);
        let adapters = block_on(instance.enumerate_adapters(Backends::VULKAN))?;
        Some(
            adapters
                .iter()
                .map(|a| {
                    let info = a.get_info();
                    tracing::info!(name = %info.name, kind = ?info.device_type, "Vulkan adapter found");
                    (info.backend, info.device_type)
                })
                .collect(),
        )
    }

    /// Runs a future that needs no reactor (adapter enumeration on native); `None` if it does
    /// not finish promptly.
    fn block_on<F: std::future::Future>(fut: F) -> Option<F::Output> {
        use std::sync::Arc;
        use std::task::{Context, Poll, Wake, Waker};
        struct Noop;
        impl Wake for Noop {
            fn wake(self: Arc<Self>) {}
        }
        let waker = Waker::from(Arc::new(Noop));
        let mut cx = Context::from_waker(&waker);
        let mut fut = std::pin::pin!(fut);
        for _ in 0..1000 {
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return Some(v);
            }
            std::thread::yield_now();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const V: Backend = Backend::Vulkan;

    #[test]
    fn real_gpu_keeps_the_default() {
        let a = [(V, DeviceType::DiscreteGpu), (V, DeviceType::Cpu)];
        assert_eq!(choose_backends(&a, None), None);
        assert_eq!(
            choose_backends(&[(V, DeviceType::IntegratedGpu)], None),
            None
        );
    }

    #[test]
    fn only_software_vulkan_falls_back_to_gl() {
        assert_eq!(
            choose_backends(&[(V, DeviceType::Cpu)], None),
            Some(Backends::GL)
        );
    }

    #[test]
    fn no_vulkan_adapter_falls_back_to_gl() {
        assert_eq!(choose_backends(&[], None), Some(Backends::GL));
        let gl = [(Backend::Gl, DeviceType::Other)];
        assert_eq!(choose_backends(&gl, None), Some(Backends::GL));
    }

    #[test]
    fn env_choice_always_wins() {
        let cpu = [(V, DeviceType::Cpu)];
        assert_eq!(choose_backends(&cpu, Some("vulkan")), None);
        assert_eq!(choose_backends(&[], Some("gl")), None);
    }

    #[test]
    fn blank_env_is_ignored() {
        let cpu = [(V, DeviceType::Cpu)];
        assert_eq!(choose_backends(&cpu, Some("  ")), Some(Backends::GL));
        assert_eq!(choose_backends(&cpu, Some("")), Some(Backends::GL));
    }
}
