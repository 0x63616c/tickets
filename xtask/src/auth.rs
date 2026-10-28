use sha2::Sha256;
use std::{
    error::Error,
    io::{self, Read},
    time::Instant,
};
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn hash() -> Result<(), Box<dyn Error>> {
    let mut password = String::new();
    io::stdin().read_to_string(&mut password)?;
    let password = password.trim_end_matches(['\r', '\n']);
    if password.len() < 12 {
        return Err("use at least 12 bytes for the bootstrap password".into());
    }
    let mut salt = [0; 16];
    getrandom::getrandom(&mut salt).map_err(|_| "randomness unavailable")?;
    let mut digest = [0; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, 600_000, &mut digest);
    println!("pbkdf2-sha256$600000${}${}", hex(&salt), hex(&digest));
    Ok(())
}
pub fn bench() -> Result<(), Box<dyn Error>> {
    let mut digest = [0; 32];
    for _ in 0..3 {
        let start = Instant::now();
        argon2::Argon2::default()
            .hash_password_into(b"benchmark password", b"benchmark salt!!", &mut digest)
            .map_err(|_| "argon2 failed")?;
        println!("Argon2id m=19456,t=2,p=1: {:?}", start.elapsed());
        let start = Instant::now();
        pbkdf2::pbkdf2_hmac::<Sha256>(
            b"benchmark password",
            b"benchmark salt!!",
            600_000,
            &mut digest,
        );
        println!("native PBKDF2-SHA256 600000: {:?}", start.elapsed());
    }
    Ok(())
}
