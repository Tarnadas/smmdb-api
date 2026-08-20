use std::{
    process::{Command, Stdio},
    str::from_utf8,
};

pub fn get_google_client_id() -> String {
    std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default()
}

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
