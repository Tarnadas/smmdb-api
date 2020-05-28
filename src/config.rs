use std::{
    process::{Command, Stdio},
    str::from_utf8,
};

pub static GOOGLE_CLIENT_ID: &str =
    "899493559187-bnvgqj1i8cnph7ilkl4h261836skee25.apps.googleusercontent.com";

pub fn _get_gateway_ip() -> String {
    let ip = match Command::new("ip")
        .args(&["route", "show", "default"])
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(process) => process,
        Err(reason) => panic!("Could not spawn 'ip' command. Reason: {}", reason),
    };
    let output = Command::new("awk")
        .arg("/default/ {print $3}")
        .stdin(ip.stdout.unwrap())
        .output()
        .unwrap_or_else(|e| panic!("failed to execute process: {}", e));
    from_utf8(&output.stdout[..]).unwrap().trim().to_owned()
}
