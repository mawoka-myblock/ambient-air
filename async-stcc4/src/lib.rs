#![no_std]
use defmt::info;
use embedded_hal_async::delay::DelayNs;
use micromath::F32Ext;
mod constants;
pub struct Stcc4<I2c, DELAYNS> {
    addr: u8,
    i2c: I2c,
    delay: DELAYNS,
    buffer: [u8; 18],
}
#[derive(Debug)]
pub enum Error<E> {
    /// I²C bus error
    I2C(E),
    Crc,
}
impl<E> From<E> for Error<E> {
    fn from(other: E) -> Self {
        Error::I2C(other)
    }
}

impl<I2C: embedded_hal_async::i2c::I2c, Delay: DelayNs> Stcc4<I2C, Delay> {
    pub fn new(addr: u8, i2c: I2C, delay: Delay) -> Self {
        Self {
            addr,
            i2c,
            delay,
            buffer: [0u8; 18],
        }
    }
    fn crc8(data: &[u8]) -> u8 {
        let mut crc = constants::CRC8_INIT;
        for &byte in data {
            crc ^= byte;
            for _ in 0..8 {
                let msb = crc & 0x80 != 0;
                crc <<= 1;
                if msb {
                    crc ^= constants::CRC8_POLYNOMIAL;
                }
            }
        }
        crc
    }

    fn check_crc<E>(&self, word: &[u8], checksum: u8) -> Result<(), Error<E>> {
        if Self::crc8(word) == checksum {
            Ok(())
        } else {
            Err(Error::Crc)
        }
    }

    async fn write_raw(&mut self, buf: &[u8]) -> Result<(), Error<I2C::Error>> {
        self.i2c.write(self.addr, buf).await?;
        Ok(())
    }

    async fn read_raw(&mut self, buf: &mut [u8]) -> Result<(), Error<I2C::Error>> {
        self.i2c.read(self.addr, buf).await?;
        Ok(())
    }

    /// Write command (16-bit)
    pub async fn write_command(&mut self, command: u16) -> Result<(), Error<I2C::Error>> {
        self.buffer[0] = (command >> 8) as u8;
        self.buffer[1] = (command & 0xFF) as u8;
        self.i2c.write(self.addr, &self.buffer[..2]).await?;
        Ok(())
    }

    /// Send a 16-bit command with a 16-bit argument
    pub async fn read_words(&mut self, _: u16, words: &mut [u16]) -> Result<(), Error<I2C::Error>> {
        // self.write_command(command)?;
        let num_bytes = words.len() * 3; // 2 data bytes + 1 CRC each

        let mut tmp = [0u8; 18];
        let buf = &mut tmp[..num_bytes];

        // safe: no borrow of `self`'s fields while calling read_raw
        self.read_raw(buf).await?;

        // copy into self.buffer if you still want to keep the data there
        self.buffer[..num_bytes].copy_from_slice(buf);

        for (i, word) in words.iter_mut().enumerate() {
            let offset = i * 3;
            let data = &self.buffer[offset..offset + 2];
            let crc_received = self.buffer[offset + 2];
            self.check_crc(data, crc_received)?;
            *word = ((data[0] as u16) << 8) | data[1] as u16;
        }

        Ok(())
    }

    /// Write a command, wait, then read returned words
    async fn delayed_read(
        &mut self,
        command: u16,
        delay_us: u32,
        words: &mut [u16],
    ) -> Result<(), Error<I2C::Error>> {
        self.write_command(command).await?;
        info!("Finished writing");
        if delay_us > 0 {
            self.delay.delay_us(delay_us).await;
        }
        info!("Now reading");
        self.read_words(command, words).await
    }

    pub async fn start_continuous(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write_command(constants::START_CONTINUOUS).await
    }

    pub async fn read_measurement_raw(
        &mut self,
    ) -> Result<(i16, u16, u16, u16), Error<I2C::Error>> {
        let mut words = [0u16; 4];
        self.delayed_read(constants::READ_MEAS_RAW, 1000, &mut words)
            .await?;
        Ok((words[0] as i16, words[1], words[2], words[3]))
    }
    pub async fn read_measurement(&mut self) -> Result<(i16, f32, f32), Error<I2C::Error>> {
        let res = self.read_measurement_raw().await?;
        let temperature_out = 175.0 * ((res.1 as f32) / ((2.0).powf(16.0) - 1.0)) - 45.0;
        let rh_out = 125.0 * (res.2 as f32 / ((2.0).powf(16.0) - 1.0)) - 6.0;
        Ok((res.0, temperature_out, rh_out))
    }

    pub async fn stop_continuous(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write_command(constants::STOP_CONTINUOUS).await
    }

    pub async fn single_shot(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write_command(constants::SINGLE_SHOT).await?;
        self.delay.delay_us(500_000).await;
        Ok(())
    }
    /*
       pub async fn forced_recalibration(
           &mut self,
           target_ppm: i16,
       ) -> Result<i16, Error<I2C::Error>> {
           let arg = target_ppm as u16;
           self.write_cmd_with_args(constants::FRC, &[arg])?;
           self.delay.delay_us(90_000).await;

           let words = self.read_words(1)?;
           Ok(words[0] as i16)
       }



       pub fn set_rht_compensation(
           &mut self,
           t_raw: u16,
           rh_raw: u16,
       ) -> Result<(), Error<I2C::Error>> {
           self.write_cmd_with_args(constants::SET_RHT_COMP, &[t_raw, rh_raw])
       }

       pub fn set_pressure_compensation(
           &mut self,
           raw_pressure: u16,
       ) -> Result<(), Error<I2C::Error>> {
           self.write_cmd_with_args(constants::SET_PRESS_COMP, &[raw_pressure])
       }

       pub async fn conditioning(&mut self) -> Result<(), Error<I2C::Error>> {
           self.write_cmd(constants::CONDITIONING)?;
           self.delay.delay_us(22_000_000).await;
           Ok(())
       }

       pub async fn sleep(&mut self) -> Result<(), Error<I2C::Error>> {
           self.write_cmd(constants::ENTER_SLEEP)?;
           self.delay.delay_us(2000).await;
           Ok(())
       }

       pub async fn wake(&mut self) -> Result<(), Error<I2C::Error>> {
           self.write_raw(&[constants::EXIT_SLEEP])?;
           self.delay.delay_us(5000).await;
           Ok(())
       }

       pub fn enable_testing(&mut self) -> Result<(), Error<I2C::Error>> {
           self.write_cmd(constants::ENABLE_TESTING)
       }

       pub fn disable_testing(&mut self) -> Result<(), Error<I2C::Error>> {
           self.write_cmd(constants::DISABLE_TESTING)
       }

       pub async fn factory_reset(&mut self) -> Result<u16, Error<I2C::Error>> {
           self.write_cmd(constants::FACTORY_RESET)?;
           self.delay.delay_us(90_000).await;
           let w = self.read_words(1)?;
           Ok(w[0])
       }
    */
    pub async fn product_id(&mut self) -> Result<(u32, u64), Error<I2C::Error>> {
        let mut words = [0u16; 6];
        self.delayed_read(constants::PRODUCT_ID, 1000, &mut words)
            .await?;

        let id = ((words[0] as u32) << 16) | words[1] as u32;

        let serial_high = ((words[2] as u32) << 16) | words[3] as u32;
        let serial_low = ((words[4] as u32) << 16) | words[5] as u32;

        let serial = ((serial_high as u64) << 32) | serial_low as u64;

        Ok((id, serial))
    }
    pub async fn self_test(&mut self) -> Result<u16, Error<I2C::Error>> {
        let mut words = [0u16; 1];
        self.delayed_read(constants::SELF_TEST, 370_000, &mut words)
            .await?;
        Ok(words[0])
    }
}
