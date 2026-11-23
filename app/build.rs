// The app's window settings and its screens (ui/dist, built by `npm run build` in ui/) go into the
// program here, as Tauri builds an app.
//
// Where Report a problem's Send goes is given to the build from outside the repository: the variable
// VPI_REPORTS_ADDRESS, else the first line of `VPI release\reports-address.txt` in the user folder of
// whoever builds it. A build given neither sends no reports: its Report a problem makes the report for
// the user to attach to a post themselves.
fn main() {
    println!("cargo:rerun-if-env-changed=VPI_REPORTS_ADDRESS");
    let file = std::env::var_os("USERPROFILE").map(|home| std::path::PathBuf::from(home).join("VPI release").join("reports-address.txt"));
    if let Some(f) = file.as_ref().filter(|f| f.is_file()) {
        println!("cargo:rerun-if-changed={}", f.display());
    }
    let address = std::env::var("VPI_REPORTS_ADDRESS")
        .ok()
        .or_else(|| file.and_then(|f| std::fs::read_to_string(f).ok()))
        .and_then(|text| text.lines().next().map(|l| l.trim().to_string()))
        .filter(|a| a.starts_with("https://"));
    if let Some(a) = address {
        println!("cargo:rustc-env=VPI_REPORTS_ADDRESS={a}");
    }
    tauri_build::build();
}
