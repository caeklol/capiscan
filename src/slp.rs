static SEGMENT_BITS: i32 = 0x7f;
static CONTINUE_BIT: i32 = 0x80;

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

    fn read_byte(&mut self) -> u8 {
        return *self.buf.get(self.cursor - 1).unwrap();
    }

    fn write_byte(&mut self, byte: u8) {
        self.cursor += 1;
        self.buf.resize(self.cursor, byte);
    }
    
    fn as_var_int(val: i32) -> Vec<u8> {
        let mut val = val;
        let mut out = Vec::new();
        loop {
            if (val & !SEGMENT_BITS) == 0 {
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

    fn read_string(&mut self) -> String {
        String::new()
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


    fn get_bytes(&self) -> Vec<u8> {
        return [Packet::as_var_int(self.buf.len().try_into().unwrap()), self.buf.clone()].concat();
    }
}

struct Result {

}

pub fn server_list_ping(addr: SocketAddr) -> Result {
    let mut packet = Packet::new();

        packet.write_byte(0);
        packet.write_var_int(760);
        packet.write_string("localhost");
        packet.write_short(25565);
        packet.write_var_int(1);

        let mut stream = TcpStream::connect("mc.hypixel.net:25565").unwrap();
        stream.write(&packet.get_bytes()).unwrap();
        stream.flush().unwrap();

        packet = Packet::new();
        packet.write_byte(0);
        stream.write(&packet.get_bytes()).unwrap();
        stream.flush().unwrap();

        packet = Packet::new();
        packet.write_byte(1);
        packet.write_long(9);
        stream.write(&packet.get_bytes()).unwrap();
        stream.flush().unwrap();


        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).unwrap();

}
