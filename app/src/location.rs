//! This PC's place, for the light of day (Settings › General, "Where you are"): Windows' own
//! location service asked once, on the user's click — Windows asks the user first when apps may
//! not use it yet — its latitude and longitude rounded to two places
//! (about a kilometre), which the page shows to be saved like any setting. Nothing else is kept,
//! and nothing goes anywhere.

/// The place Windows gives, or why it can't: latitude and longitude, north and east.
#[cfg(windows)]
pub fn this_pc() -> Result<(f64, f64), String> {
    use windows::Devices::Geolocation::{GeolocationAccessStatus, Geolocator};
    use windows::Foundation::TimeSpan;
    let off = "Windows won’t give apps this PC’s location: turn it on in Windows’ Settings › Privacy & security › Location (“Let desktop apps access your location”), or type your place’s latitude and longitude.";
    let access = Geolocator::RequestAccessAsync().and_then(|a| a.join()).map_err(|e| format!("Windows’ location service: {e}"))?;
    if access != GeolocationAccessStatus::Allowed {
        return Err(off.to_string());
    }
    // a place found in the last hour will do, and no more than half a minute's wait for one
    let (max_age, timeout) = (TimeSpan { Duration: 36_000_000_000 }, TimeSpan { Duration: 300_000_000 });
    let found = Geolocator::new()
        .and_then(|g| g.GetGeopositionAsyncWithAgeAndTimeout(max_age, timeout))
        .and_then(|a| a.join())
        .and_then(|p| p.Coordinate())
        .and_then(|c| c.Point())
        .and_then(|p| p.Position())
        .map_err(|e| format!("Windows didn’t find this PC’s location: {}", e.message()))?;
    let round = |v: f64| (v * 100.0).round() / 100.0;
    Ok((round(found.Latitude), round(found.Longitude)))
}

#[cfg(not(windows))]
pub fn this_pc() -> Result<(f64, f64), String> {
    Err("Only Windows can say where this PC is.".into())
}
