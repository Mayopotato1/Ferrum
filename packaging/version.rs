//! Windows version policy: beta.N uses revision N; stable uses 65535 to sort
//! after its betas. SemVer remains the customer-visible version everywhere.
pub fn numeric(version: &str) -> Result<[u16; 4], String> {
    let (core, revision) = if let Some((core, beta)) = version.split_once("-beta.") {
        let n: u16 = beta.parse().map_err(|_| "Invalid beta sequence")?;
        if n == 0 || n == u16::MAX {
            return Err("Beta sequence must be 1..65534".into());
        }
        (core, n)
    } else {
        (version, u16::MAX)
    };
    let parts: Vec<_> = core.split('.').collect();
    if parts.len() != 3 {
        return Err("Expected major.minor.patch[-beta.N]".into());
    }
    Ok([
        parts[0].parse().map_err(|_| "Invalid major")?,
        parts[1].parse().map_err(|_| "Invalid minor")?,
        parts[2].parse().map_err(|_| "Invalid patch")?,
        revision,
    ])
}
