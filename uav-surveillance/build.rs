fn main() {
    dotenvy::dotenv().ok();
    println!(
        "cargo:rustc-env=WIFI_SSID={}",
        std::env::var("WIFI_SSID").unwrap()
    );
    println!(
        "cargo:rustc-env=WIFI_PASS={}",
        std::env::var("WIFI_PASS").unwrap()
    );
    embuild::espidf::sysenv::output();
}
