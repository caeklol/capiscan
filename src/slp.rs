use std::{io::{Read, Write}, net::{IpAddr, SocketAddr, TcpStream}, num::TryFromIntError, str::FromStr, string::FromUtf8Error, time::{SystemTime, UNIX_EPOCH}};

use anyhow::{ensure, Error};

static SEGMENT_BITS: i32 = 0x7f;
static CONTINUE_BIT: i32 = 0x80;

#[derive(thiserror::Error, Debug)]
pub enum CursorError {
    #[error("Cursor went out of bounds")]
    OutOfBounds {
        cursor: usize
    }
}

#[derive(thiserror::Error, Debug)]
pub enum VarIntError {
    #[error("Cursor error while parsing VarInt")]
    Cursor(CursorError),
    #[error("VarInt is over 3 bytes")]
    Oversized 
}

#[derive(thiserror::Error, Debug)]
pub enum StringError {
    #[error("VarInt failed to parse")]
    VarInt (VarIntError),
    #[error("Cursor error while parsing String")]
    Cursor (CursorError),
    #[error("Invalid string data (not UTF-8?)")]
    StringData (FromUtf8Error),
    #[error("Length is unreasonable (negative / oversize)")]
    UnreasonableLength,
}

#[derive(thiserror::Error, Debug)]
pub enum PacketIdError {
    #[error("Cursor error while parsing Packet ID")]
    Cursor (CursorError),
    #[error("Packet ID does not match expected value")]
    Invalid
}

#[derive(thiserror::Error, Debug)]
pub enum PacketLengthError {
    #[error("Packet length field is unreasonable (negative / oversize)")]
    UnreasonableLength,
    #[error("Packet length field does not match packet length")]
    Length(CursorError),
    #[error("Packet length field is an invalid VarInt")]
    VarInt (VarIntError)
}

#[derive(thiserror::Error, Debug)]
pub enum PacketParseError {
    #[error("Failed to parse a String")]
    String (#[from] StringError),
    #[error("Invalid packet ID")]
    PacketId(#[from] PacketIdError),
    #[error("Invalid packet length")]
    PacketLength(#[from] PacketLengthError),
    #[error("Invalid packet data")]
    PacketData
}

#[derive(thiserror::Error, Debug)]
pub enum SlpError {
    #[error("Failed to ping")]
    IO(#[from] std::io::Error),
    #[error("Failed to parse response")]
    Response(#[from] PacketParseError)
}

struct Packet {
    buf: Vec<u8>,
    cursor: usize
}

impl From<Vec<u8>> for Packet {
    fn from(bytes: Vec<u8>) -> Self {
        Self { buf: bytes, cursor: 0 }
    }
}

impl Packet {
    fn new() -> Self {
        Self { buf: Vec::new(), cursor: 0 }
    }

    fn write_byte(&mut self, byte: u8) {
        self.cursor += 1;
        self.buf.resize(self.cursor, byte);
    }
    
    fn as_var_int(val: i32) -> Vec<u8> {
        let mut val = val;
        let mut out = Vec::new();
        loop {
            if (val & !SEGMENT_BITS as i32) == 0 {
                out.push(val.try_into().unwrap());
                return out;
            }

            out.push(((val & SEGMENT_BITS) | CONTINUE_BIT).try_into().unwrap());

            val = val.wrapping_shr(7);
        }
    }

    fn write_var_int(&mut self, val: i32) {
        for byte in Packet::as_var_int(val) {
            self.write_byte(byte);
        }
    }

    fn write_string(&mut self, str: &str) {
        self.write_var_int(str.len().try_into().unwrap());
        for byte in str.as_bytes() {
            self.write_byte(*byte); 
        }
    } 

    fn write_short(&mut self, short: u16) {
        for byte in short.to_be_bytes() {
            self.write_byte(byte);
        }
    }

    fn write_long(&mut self, long: i64) {
        for byte in long.to_be_bytes() {
            self.write_byte(byte);
        }
    }

    fn read_byte(&mut self) -> Result<&u8, CursorError> {
        self.cursor += 1;
        return self.buf.get(self.cursor - 1).ok_or(CursorError::OutOfBounds {
            cursor: self.cursor
        });
    }

    fn read_bytes(&mut self, bytes: usize) -> Result<Vec<u8>, CursorError> {
        let mut out = Vec::new();
        for _ in 0..bytes {
            out.push(*self.read_byte()?);
        }
        return Ok(out);
    }

    fn read_var_int(&mut self) -> Result<i32, VarIntError> {
        let mut value: i32 = 0;
        let mut position = 0;
        let mut current_byte;

        loop {
            current_byte = *self.read_byte().map_err(|e| VarIntError::Cursor(e))?;
            value |= (current_byte as i32 & SEGMENT_BITS) << position;

            if (current_byte as i32 & CONTINUE_BIT) == 0 {
                break;
            }

            position += 7;

            if position >= 32 {
                return Err(VarIntError::Oversized);
            }
        }

        return Ok(value);
    }

    fn read_string(&mut self) -> Result<String, StringError> {
        let length = self.read_var_int().map_err(|e| StringError::VarInt(e))?;
        let str_bytes = self.read_bytes(length.try_into().ok().ok_or(StringError::UnreasonableLength)?)
            .map_err(|e| StringError::Cursor(e))?;
        
        return match String::from_utf8(str_bytes) {
            Ok(str) => return Ok(str),
            Err(e) => Err(StringError::StringData(e)),
        };
    }

    fn get_bytes(&self) -> Vec<u8> {
        return [Packet::as_var_int(self.buf.len().try_into().unwrap()), self.buf.clone()].concat();
    }
}

#[derive(Debug)]
pub struct StatusResponse {
    
}

pub fn server_list_ping(addr: SocketAddr) -> Result<StatusResponse, SlpError> {
    let mut packet = Packet::new();

    // handshake
    packet.write_byte(0);
    packet.write_var_int(760);
    packet.write_string(&addr.ip().to_string());
    //packet.write_string("mc.hypixel.net");
    packet.write_short(addr.port());
    packet.write_var_int(1);

    let mut stream = TcpStream::connect(addr)?;
    stream.write(&packet.get_bytes())?;
    stream.flush()?;

    // status request
    packet = Packet::new();
    packet.write_byte(0);
    stream.write(&packet.get_bytes())?;
    stream.flush()?;

    // ping request
    packet = Packet::new();
    packet.write_byte(1);
    packet.write_long(SystemTime::now().duration_since(UNIX_EPOCH).expect("time is moving backward").as_millis().try_into().expect("we are too far into the future"));
    //packet.write_long(1752829549176);
    stream.write(&packet.get_bytes())?;
    stream.flush()?;


    let mut buf = Vec::new();
    stream.read_to_end(&mut buf)?;

    let mut frame = Packet::from(buf);
    let length = frame.read_var_int().map_err(|e| PacketParseError::PacketLength(PacketLengthError::VarInt(e)))?;

    let mut packet = Packet::from(
        frame.read_bytes(length.try_into().ok().ok_or(PacketParseError::PacketLength(PacketLengthError::UnreasonableLength))?)
            .map_err(|e| PacketParseError::PacketLength(PacketLengthError::Length(e)))?
    );

    if *packet.read_byte().map_err(|e| PacketParseError::PacketId(PacketIdError::Cursor(e)))? != 0 {
        return Err(PacketParseError::PacketId(PacketIdError::Invalid).into());
    }

    let json_str = packet.read_string().map_err(|e| PacketParseError::String(e))?;
    println!("{}", json_str);

    Ok(StatusResponse {  })
}
