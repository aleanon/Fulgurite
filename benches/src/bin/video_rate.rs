//! **M84's K84e** (`docs/plan-images-in-motion.md`, `docs/results-images.md`
//! T84.8): a 1280 × 720 `GifFeed` stream at 30 frames a second, on the
//! software backend — how many frames does the window present per second?
//!
//! The window's loop, headless: a `Host` with a `Video` whose picture is
//! 1280 × 720, a real worker (`Threads`) running the feed as a
//! subscription, and a software `Canvas` presenting each frame's damage
//! with the application's store first (`render_within_from`). The loop
//! pumps when a message is due and presents when the pump left damage, as
//! the window does on its waker; a present is counted per frame whose
//! damage it drew. Ten seconds, printed per second and overall, against
//! K84e's bound of 28.
//!
//! The source is three 1280 × 720 frames at 30, 40 and 30 ms (GIF delays
//! are hundredths, so 30/s is three frames per 100 ms), built in memory: a
//! decoded GIF, which is all a `GifFeed` reads.
//!
//! Run: `cargo run --release -p benches --features image --bin video_rate`
//! (`-- 640 360` for another size).

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use lumen_static::display::ImageId;
use lumen_static::prelude::*;
use lumen_static::retain::Damage;
use lumen_static::spawn::{Descriptor, JobFn, Subs, Threads};
use lumen_static_shell::image::{DecodedImage, ImageStore};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::video::{video_job, GifFeed, GifLibrary, VideoFrame, VideoKey};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

static SIZE: OnceLock<(u32, u32)> = OnceLock::new();

struct Big;

impl GifLibrary for Big {
    fn gif(index: u16) -> Option<&'static DecodedImage> {
        static GIF: OnceLock<DecodedImage> = OnceLock::new();
        let (w, h) = *SIZE.get().expect("size set");
        (index == 0).then(|| {
            GIF.get_or_init(|| DecodedImage {
                w,
                h,
                // A gradient per frame, so no two frames share pixels.
                frames: (0..3u32)
                    .map(|k| {
                        let mut f = Vec::with_capacity((w * h * 4) as usize);
                        for y in 0..h {
                            for x in 0..w {
                                f.extend_from_slice(&[(x * 255 / w) as u8, (y * 255 / h) as u8, (k * 90) as u8, 255]);
                            }
                        }
                        f
                    })
                    .collect(),
                delays_ms: vec![30, 40, 30],
            })
        })
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    v: Video<St>,
}

#[derive(Default)]
struct St {
    store: ImageStore,
    frame: u32,
    position_ms: u32,
    subs: Subs,
    applied: u64,
}

#[derive(DeriveApp, Default)]
#[app(msg = VideoFrame, apply = take, images = images, subscriptions = subs, jobs = jobs)]
#[app(after_update = refresh)]
struct Player {
    #[tree]
    ui: Ui,
    #[state]
    st: St,
}

fn take(s: &mut St, f: VideoFrame) -> bool {
    match s.store.replace(ImageId(1), f.w, f.h, &f.rgba) {
        Some(n) => {
            s.frame = n;
            s.position_ms = f.position_ms;
            s.applied += 1;
            true
        }
        None => false,
    }
}

fn images(p: &Player) -> Option<&dyn lumen_static::ImageSource> {
    Some(&p.st.store)
}

fn subs(p: &mut Player) -> Option<&mut Subs> {
    Some(&mut p.st.subs)
}

fn jobs(d: Descriptor) -> Option<JobFn<VideoFrame>> {
    (d.kind == 1).then_some(video_job::<GifFeed<Big>, VideoFrame> as JobFn<VideoFrame>)
}

fn refresh(p: &mut Player) -> bool {
    let v = &mut p.ui.v;
    let changed = v.frame != p.st.frame || v.position_ms != p.st.position_ms;
    v.frame = p.st.frame;
    v.position_ms = p.st.position_ms;
    changed
}

fn main() {
    let args: Vec<u32> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let (w, h) = (args.first().copied().unwrap_or(1280), args.get(1).copied().unwrap_or(720));
    SIZE.set((w, h)).expect("once");
    let (ww, wh) = (w + 20, h + 20 + lumen_static_widgets::video::CONTROLS_H as u32);
    let mut p = Player::default();
    p.ui.v = Video::new(ImageId(1), Size { w: w as f32, h: h as f32 });
    p.ui.v.duration_ms = 100;
    p.ui.v.playing = true;
    let mut host: Host<Player> = Host::new(p, ww, wh);
    host.set_spawner(Box::new(Threads::new()));
    host.app.st.subs.declare(1, VideoKey { source: 0, seek: 0, start_ms: 0 }.pack());
    let mut canvas = Canvas::new(ww, wh);
    let mut per_sec = Vec::new();
    let (mut presents, mut raster) = (0u64, Duration::ZERO);
    let t0 = Instant::now();
    let mut next = t0 + Duration::from_secs(1);
    let mut last = 0u64;
    while t0.elapsed() < Duration::from_secs(10) {
        host.pump();
        let damage = host.damage();
        if !matches!(damage, Damage::None) {
            let rect = match damage {
                Damage::Regions(r) => r.bounds().unwrap_or([0.0, 0.0, ww as f32, wh as f32]),
                _ => [0.0, 0.0, ww as f32, wh as f32],
            };
            let tr = Instant::now();
            canvas.render_within_from(&host.list, rect, lumen_static::event::App::images(&host.app));
            raster += tr.elapsed();
            presents += 1;
        } else {
            // The window sleeps on its waker; a millisecond's poll is close
            // enough for a count per second.
            std::thread::sleep(Duration::from_millis(1));
        }
        if Instant::now() >= next {
            per_sec.push(presents - last);
            last = presents;
            next += Duration::from_secs(1);
        }
    }
    let secs = t0.elapsed().as_secs_f64();
    let rate = presents as f64 / secs;
    println!(
        "K84e: {w}x{h} GifFeed at 30/s, software: {presents} presents in {secs:.2} s = {rate:.1}/s \
         (bound >= 28); per second {per_sec:?}; frames applied {}; inbox refused {}; raster {:.2} ms/present",
        host.app.st.applied,
        host.inbox.refused(),
        raster.as_secs_f64() * 1000.0 / presents.max(1) as f64
    );
}
