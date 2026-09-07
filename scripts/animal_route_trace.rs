//! Small CPU fixture for choosing inspectable windows on real dog routes.
//! Compile against the same cathedral_sim rlib as the animal studio.
use cathedral_sim::{NavData, dogs};

fn main() {
    let name = std::env::args().nth(1).unwrap_or_else(|| "Bracken".into());
    let nav = NavData::from_parts(
        &std::fs::read_to_string("assets/world/navigation.json").unwrap(),
        &std::fs::read("assets/world/navigation.bin").unwrap(),
    ).unwrap();
    let mut pack = dogs::seed_pack(&nav);
    pack.retain(|dog| dog.name == name);
    assert_eq!(pack.len(), 1, "select one authored dog by name");
    println!("time_seconds,speed_mps,x_m,z_m,heading_yaw,gait_phase");
    for step in 0..2400 {
        dogs::step_dogs(&mut pack, 0.05, &nav);
        let dog = &pack[0];
        println!("{:.2},{:.8},{:.8},{:.8},{:.8},{:.8}",
            (step + 1) as f64 * 0.05, dog.speed, dog.position_m.x,
            dog.position_m.z, dog.facing_yaw, dog.gait_phase);
    }
}
