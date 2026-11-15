//! The iPad app: `orb-ui`'s panel and `orb-player`'s callback, run by winit's UIKit backend.
//!
//! The desktop binary (`orb-ui`) parses arguments, opens the song named on the command line and
//! builds the window. The iPad has neither a command line nor a mod folder to point at, so this
//! shell starts on the new song with the built-in kit ("Standard ReBirth"), and its files are
//! the app's Documents folder, which `UIFileSharingEnabled` shows in Files.
//!
//! Everything the app needs is built in the app creator, not in `run`: `main` must reach
//! `UIApplicationMain` at once, because iOS's launch watchdog counts from process start and a
//! launch that is still loading the mod, the tables and CoreAudio is a launch that died.
//!
//! `scripts/ios-app.sh` builds the crate for `aarch64-apple-ios`, wraps the binary in
//! `OpenRB-338.app`, signs it with a development profile and installs it on the iPad.
#![cfg_attr(not(target_os = "ios"), allow(dead_code, unused_imports))]

#[cfg(target_os = "ios")]
use std::sync::Arc;

#[cfg(target_os = "ios")]
use eframe::egui;
#[cfg(target_os = "ios")]
use orb_engine::tables::Tables;
#[cfg(target_os = "ios")]
use orb_ui::app::App;
#[cfg(target_os = "ios")]
use orb_ui::link::Link;
#[cfg(target_os = "ios")]
use orb_ui::model::Panel;
#[cfg(target_os = "ios")]
use orb_ui::settings::Settings;

/// Why the app speaks through a file: a device launch hands back the exit code and no stderr,
/// so every step of the start writes `Documents/boot.log`, which `devicectl device copy from`
/// fetches. The milliseconds since the start say how long each step took.
#[cfg(target_os = "ios")]
mod boot {
    use std::fs::File;
    use std::io::Write;
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;

    static LOG: OnceLock<Mutex<Option<File>>> = OnceLock::new();
    static START: OnceLock<Instant> = OnceLock::new();

    pub fn start() {
        let _ = START.set(Instant::now());
        let file = std::env::var_os("HOME").and_then(|home| {
            let dir = std::path::PathBuf::from(home).join("Documents");
            std::fs::create_dir_all(&dir).ok()?;
            File::create(dir.join("boot.log")).ok()
        });
        let _ = LOG.set(Mutex::new(file));
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            note(&format!("panic at {info}"));
            note(&std::backtrace::Backtrace::force_capture().to_string());
            default(info);
        }));
        let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Info));
    }

    pub fn note(line: &str) {
        let ms = START.get().map_or(0, |s| s.elapsed().as_millis());
        if let Some(log) = LOG.get()
            && let Ok(mut log) = log.lock()
            && let Some(file) = log.as_mut()
        {
            let _ = writeln!(file, "{ms:>6} ms {line}");
            let _ = file.flush();
        }
    }

    /// The `log` sink: eframe, winit and wgpu say what went wrong only through it.
    struct Sink;

    static LOGGER: Sink = Sink;

    impl log::Log for Sink {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }

        fn log(&self, record: &log::Record) {
            note(&format!(
                "{} {}: {}",
                record.level(),
                record.target(),
                record.args()
            ));
        }

        fn flush(&self) {}
    }
}

#[cfg(target_os = "ios")]
unsafe extern "C" {
    /// `src/audio_session.m`: the Playback category, 44.1 kHz, and the session active.
    fn orb_ios_audio_session();
    /// `src/audio_session.m`: puts winit's window in the app's window scene.
    fn orb_ios_attach_scene();
}

/// The window's options, with the device limits the iPad's Metal adapter actually has: egui
/// asks for `wgpu::Limits::default()` (16 inter-stage shader variables), the adapter allows 15,
/// and a limit above the adapter's is a hard startup error, not a fallback.
#[cfg(target_os = "ios")]
fn native_options() -> eframe::NativeOptions {
    use eframe::egui_wgpu::wgpu;
    use eframe::egui_wgpu::{WgpuSetup, WgpuSetupCreateNew};

    let device_descriptor = std::sync::Arc::new(|adapter: &wgpu::Adapter| wgpu::DeviceDescriptor {
        label: Some("OpenRB-338"),
        required_limits: wgpu::Limits {
            max_texture_dimension_2d: 8192,
            ..adapter.limits()
        },
        ..Default::default()
    });
    let mut options = eframe::NativeOptions::default();
    options.wgpu_options.wgpu_setup = WgpuSetup::CreateNew(WgpuSetupCreateNew {
        device_descriptor,
        ..WgpuSetupCreateNew::without_display_handle()
    });
    options
}

#[cfg(target_os = "ios")]
pub fn run() {
    boot::start();
    boot::note("start");
    let result = eframe::run_native(
        "OpenRB-338",
        native_options(),
        Box::new(|cc| {
            boot::note("creating the app");
            // SAFETY: the app creator runs on the main thread, before cpal opens the unit.
            unsafe { orb_ios_audio_session() };
            cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
            let samples = Some(rbm::standard::builtin_samples());
            boot::note("samples");
            let (link, audio) = Link::new(Arc::new(Tables::new()), samples.clone());
            boot::note("link");
            let stream = match orb_player::output::start(audio) {
                Ok(s) => {
                    boot::note(&format!("audio: {} {:?}", s.device, s.config));
                    Some(s)
                }
                // The panel runs without sound, as the desktop one does.
                Err(e) => {
                    boot::note(&format!("audio: {e}"));
                    None
                }
            };
            let mut app = App::with_link(Panel::new(), link, None);
            // The kit the drum machines play; without it their pattern blocks are
            // placeholders and `start`'s new song has none to show.
            app.samples = samples;
            app.menu = true;
            app.settings = Settings::load();
            app.start(None);
            app.enable_logo_anim();
            boot::note("app created");
            // SAFETY: main thread; the window winit made is a plain UIWindow until this.
            unsafe { orb_ios_attach_scene() };
            Ok(Box::new(Shell {
                app,
                frames: 0,
                _stream: stream,
            }))
        }),
    );
    boot::note(&format!("run_native returned: {result:?}"));
    if let Err(e) = result {
        panic!("the panel did not start: {e}");
    }
}

/// The panel and the stream that must outlive it: dropping the stream stops the sound.
#[cfg(target_os = "ios")]
struct Shell {
    app: App,
    frames: u64,
    _stream: Option<orb_player::output::Stream>,
}

#[cfg(target_os = "ios")]
impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if self.frames == 0 {
            boot::note("first frame");
            // SAFETY: main thread; the scene may have connected after the window was made.
            unsafe { orb_ios_attach_scene() };
        }
        self.frames += 1;
        eframe::App::ui(&mut self.app, ui, frame);
    }
}
