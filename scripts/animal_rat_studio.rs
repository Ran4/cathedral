// Capture adapter; the geometry and motion implementations above this file are
// verbatim production source slices. The legacy branch keeps baseline captures
// reproducible after the production pose API changes.
#[derive(Resource)]
pub struct StudioState {
    rat: Rat,
    #[cfg(animal_rat_motion)]
    pose: RatPose,
    position: Vec2,
    heading: Vec2,
    elapsed: f32,
    moving: bool,
}

impl Default for StudioState {
    fn default() -> Self {
        let number = |name: &str, fallback: f32| std::env::var(name)
            .ok().map(|s| s.parse::<f32>().unwrap()).unwrap_or(fallback);
        Self::from_rat(Rat {
            seed: std::env::var("ANIMAL_RAT_SEED").ok().map(|s| s.parse().unwrap()).unwrap_or(37),
            legs: Vec::new(), period: number("ANIMAL_RAT_PERIOD", 10.0),
            phase: number("ANIMAL_RAT_PHASE", 0.0),
            length_m: number("ANIMAL_RAT_LENGTH_M", 0.28),
            tint: number("ANIMAL_RAT_TINT", 1.0),
            #[cfg(animal_rat_motion)]
            motion: RatMotion::default(),
        })
    }
}

impl StudioState {
    #[allow(unused_mut)]
    fn from_rat(mut rat: Rat) -> Self {
        #[cfg(animal_rat_motion)]
        let pose = rat.motion.update(rat.seed, rat.length_m, Vec2::ZERO,
            -Vec2::Y, 0.0, f32::INFINITY);
        Self { rat, #[cfg(animal_rat_motion)] pose, position: Vec2::ZERO,
            heading: -Vec2::Y, elapsed: 0.0, moving: false }
    }

    pub fn update(&mut self, position: Vec2, heading: Vec2, speed: f32,
        elapsed: f32, pause_remaining: f32) {
        self.position = position;
        self.heading = heading;
        self.elapsed = elapsed;
        self.moving = speed > 0.01;
        let _ = pause_remaining;
        #[cfg(animal_rat_motion)]
        { self.pose = self.rat.motion.update(self.rat.seed, self.rat.length_m,
            position, heading, elapsed, pause_remaining); }
    }

    pub fn reset_at(&mut self, position: Vec2, heading: Vec2, pause_remaining: f32) {
        #[cfg(animal_rat_motion)]
        { self.rat.motion = RatMotion::default(); }
        self.update(position, heading, 0.0, 0.0, pause_remaining);
    }

    fn append(&self, p: &mut Vec<[f32; 3]>, n: &mut Vec<[f32; 3]>,
        u: &mut Vec<[f32; 2]>, c: &mut Vec<[f32; 4]>, i: &mut Vec<u32>) {
        #[cfg(animal_rat_motion)]
        push_rat(p, n, u, c, i, &self.rat, self.position, &self.pose);
        #[cfg(not(animal_rat_motion))]
        push_rat(p, n, u, c, i, &self.rat, self.position, self.heading, self.moving, self.elapsed);
    }

    pub fn mesh(&self) -> Mesh {
        let (mut p, mut n, mut u, mut c, mut i) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        self.append(&mut p, &mut n, &mut u, &mut c, &mut i);
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_indices(Indices::U32(i))
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, p)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, n)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, u)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, c)
    }

    pub fn metadata(&self) -> String {
        #[cfg(animal_rat_motion)]
        {
            let ahead = Vec3::new(self.pose.heading.x, 0.0, self.pose.heading.y);
            let side = Vec3::new(-self.pose.heading.y, 0.0, self.pose.heading.x);
            let center = Vec3::new(self.position.x, RAT_GROUND_Y, self.position.y);
            let feet = self.pose.feet.map(|p| (center + ahead * p.x + Vec3::Y * p.y + side * p.z).to_array());
            return format!("{{\"position_xz\":{:?},\"heading_hint\":{:?},\"legacy\":false,\"heading\":{:?},\"speed_mps\":{},\"phase\":{},\"travel\":{},\"action\":\"{:?}\",\"sniff\":{},\"alert\":{},\"groom\":{},\"head_yaw\":{},\"ears\":{:?},\"tail_bend\":{},\"foot_anchors_world\":{:?},\"paw_curl\":{:?}}}",
                self.position.to_array(), self.heading.to_array(), self.pose.heading.to_array(),
                self.pose.speed, self.pose.phase, self.pose.travel, self.pose.action,
                self.pose.sniff, self.pose.alert, self.pose.groom, self.pose.head_yaw,
                self.pose.ears, self.pose.tail_bend, feet, self.pose.paw_curl);
        }
        #[cfg(not(animal_rat_motion))]
        format!("{{\"position_xz\":{:?},\"heading_hint\":{:?},\"legacy\":{}}}",
            self.position.to_array(), self.heading.to_array(), !cfg!(animal_rat_motion))
    }
}

pub fn studio_benchmark() -> String {
    let mut rats: Vec<StudioState> = (0..50).map(|index| {
        let seed = mix(37 + index);
        StudioState::from_rat(Rat { seed, legs: Vec::new(), period: 10.0,
            phase: unit(seed, 31) * 10.0, length_m: 0.24 + 0.08 * unit(seed, 32),
            tint: 0.8 + 0.35 * unit(seed, 33),
            #[cfg(animal_rat_motion)] motion: RatMotion::default() })
    }).collect();
    let (mut p, mut n, mut u, mut c, mut i) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut samples = Vec::new();
    let (mut allocations_total, mut allocations_max) = (0, 0);
    for frame in 0..1200 {
        p.clear(); n.clear(); u.clear(); c.clear(); i.clear();
        crate::perf::take_allocations();
        let tracked = crate::perf::span(crate::perf::Probe::Rats);
        let start = std::time::Instant::now();
        let elapsed = frame as f32 / 60.0;
        let speed = if frame < 650 { 1.7 } else { 0.0 };
        for (index, rat) in rats.iter_mut().enumerate() {
            rat.update(Vec2::new(index as f32 * 0.4, -1.7 * frame.min(650) as f32 / 60.0),
                -Vec2::Y, speed, elapsed, if speed > 0.0 { 0.0 } else { f32::INFINITY });
            rat.append(&mut p, &mut n, &mut u, &mut c, &mut i);
        }
        std::hint::black_box((&p, &n, &u, &c, &i));
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        drop(tracked);
        let allocations = crate::perf::take_allocations();
        if frame >= 200 {
            samples.push(elapsed);
            allocations_total += allocations;
            allocations_max = allocations_max.max(allocations);
        }
    }
    samples.sort_by(f64::total_cmp);
    format!("{{\"count\":50,\"samples\":{},\"p50_ms\":{},\"p95_ms\":{},\"p99_ms\":{},\"vertices\":{},\"triangles\":{},\"geometry_allocations_total\":{},\"geometry_allocations_max_per_frame\":{}}}",
        samples.len(), samples[500], samples[950], samples[990], p.len(), i.len() / 3, allocations_total, allocations_max)
}
