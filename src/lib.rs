extern crate serial;


pub mod slcan {

    pub use std::io::prelude::*;
    pub use serial::prelude::*;

    use std::io;
    use std::iter;
    use std::mem;
    use std::ops;
    use std::str;
    use std::ascii::AsciiExt;

    enum Bitrate {
        Br10Kbps = 0,
        Br20Kbps = 1,
        Br50Kbps = 2,
        Br100Kbps = 3,
        Br125Kbps = 4,
        Br250Kbps = 5,
        Br500Kbps = 6,
        Br800Kbps = 7,
        Br1000Kbps = 8,
    }

    struct CanMsg {
        is_eff: bool,
        is_rtr: bool,
        id: u32,
        data: Vec<u8>,
    }

    fn num_to_string(bytes: &[u8]) -> String {
        let hex_chars = "01234567890ABCDEF".as_bytes();
        let mut string = String::from("");

        for byte in bytes {
            string.push(hex_chars[(byte >> 4) as usize] as char);
            string.push(hex_chars[(byte & 0x0F) as usize] as char);
        }

        string
    }

    fn string_to_num(str_slice: &str) -> io::Result<Vec<u8>> {
        let hex_chars = "01234567890ABCDEF";
        let mut bytes: Vec<u8> = vec![];

        if str_slice.is_ascii() {
            for ch in str_slice.as_bytes() {

                let mut byte = hex_chars.find((ch >> 4) as char)
                    .ok_or(io::Error::from(io::ErrorKind::InvalidInput))?;
                byte <<= 4;
                byte &= hex_chars.find((ch & 0x0F) as char)
                    .ok_or(io::Error::from(io::ErrorKind::InvalidInput))?;
                bytes.push(byte as u8);
            }
        }

        Ok(bytes)
    }

    fn u32_to_bytes(num: u32) -> Vec<u8> {
        vec![(num >> 24) as u8, (num >> 16) as u8, (num >> 8) as u8, (num >> 0) as u8]
    }

    fn bytes_to_u32(bytes: Vec<u8>) -> u32 {
        ((bytes[0] as u32) << 24) | ((bytes[1] as u32) << 16) | ((bytes[2] as u32) << 8) |
        (bytes[3] as u32)
    }

    struct Slcan<'a> {
        port: &'a mut SerialPort,
        buf: [u8; 32],
        buf_size: usize,
    }

    impl<'a> Slcan<'a> {
        pub fn new(port: &'a mut SerialPort) -> Slcan {
            Slcan {
                port: port,
                buf: [0; 32],
                buf_size: 0,
            }
        }

        pub fn setup_bitrate(&mut self, bitrate: Bitrate) -> io::Result<()> {
            let br_code = String::from("S") + (bitrate as isize).to_string().as_str() + "\r";
            self.exec_command(&br_code[..])?;

            Ok(())
        }

        pub fn open(&mut self) -> io::Result<()> {
            self.exec_command("O\r")?;

            Ok(())
        }

        pub fn close(&mut self) -> io::Result<()> {
            self.exec_command("C\r")?;

            Ok(())
        }

        pub fn set_timestamp(&mut self, on: bool) -> io::Result<()> {
            self.exec_command(if on { "Z1\r" } else { "Z0\r" })?;

            Ok(())
        }

        pub fn set_acceptance_mask(&mut self, mask: u32) -> io::Result<()> {
            let bytes = u32_to_bytes(mask);
            let mask_code = String::from("M") + num_to_string(&bytes[..]).as_str();

            self.exec_command(&mask_code[..])?;

            Ok(())
        }

        pub fn set_acceptance_id(&mut self, id: u32) -> io::Result<()> {
            let bytes = u32_to_bytes(id);
            let id_code = String::from("m") + num_to_string(&bytes[..]).as_str();

            self.exec_command(&id_code[..])?;

            Ok(())
        }

        pub fn read(&mut self) -> io::Result<CanMsg> {
            self.buf_size = self.port.read(self.buf.as_mut())?;

            if (self.buf_size < 6) || (self.buf[self.buf_size - 1] != "\r".as_bytes()[0]) {
                Err(io::Error::from(io::ErrorKind::InvalidData))?;
            }

            let (is_eff, is_rtr) = match self.buf[0] as char {
                't' => Ok((false, false)),
                'T' => Ok((true, false)),
                'r' => Ok((false, true)),
                'R' => Ok((true, true)),
                _ => Err(io::Error::from(io::ErrorKind::InvalidData)),
            }?;

            let id_end = if is_eff { 9 } else { 4 };
            let id_str = str::from_utf8(&self.buf[1..id_end]).map_err(|e| io::Error::from(io::ErrorKind::InvalidData))?;
            let id = bytes_to_u32(string_to_num(id_str)?);
            let dlc = self.buf[id_end];
            let data_str = str::from_utf8(&self.buf).map_err(|e| io::Error::from(io::ErrorKind::InvalidData))?;
            let data = string_to_num(data_str)?;

            Ok(CanMsg {
                is_eff,
                is_rtr,
                id,
                data,
            })
        }

        pub fn write(&mut self, msg: CanMsg) -> io::Result<()> {
            let id_bytes = u32_to_bytes(msg.id);
            let id_code = num_to_string(&u32_to_bytes(msg.id));
            let offset = if msg.is_eff { 0 } else { 5 };

            let mut str_msg = match (msg.is_eff, msg.is_rtr) {
                (false, false) => String::from("t"),
                (true, false) => String::from("T"),
                (false, true) => String::from("r"),
                (true, true) => String::from("R"),
            };

            str_msg += id_code.as_str();
            str_msg += (msg.data.len() as u8 as char).to_string().as_str();
            str_msg += num_to_string(&msg.data[..]).as_str();


            self.exec_command(&str_msg)?;

            Ok(())
        }

        fn exec_command(&mut self, cmd: &str) -> io::Result<()> {
            self.port.write(cmd.as_bytes());
            self.buf_size = self.port.read(self.buf.as_mut())?;

            if self.buf[self.buf_size - 1] == '\r' as u8 {
                Ok(())
            } else {
                Err(io::Error::from(io::ErrorKind::TimedOut))
            }
        }
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn it_works() {}
    }

}
