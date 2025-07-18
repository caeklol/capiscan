use std::{io::{Read, Write}, net::{IpAddr, TcpStream}, str::FromStr};

fn main() {
    println!("{:?}", server_list_ping("mc.hypixel.net", "25565"));
}
