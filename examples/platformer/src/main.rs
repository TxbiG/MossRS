use moss::prelude::*;

fn main() -> Result<()> {
    let mut engine = Engine::new()?;

    for _ in 0..3 {
        engine.update(1.0 / 60.0)?;
        engine.render()?;
    }

    println!("MossRS core bridge initialized successfully.");
    println!("Vec3 example: {:?}", Vec3::new(1.0, 2.0, 3.0));

    Ok(())
}
