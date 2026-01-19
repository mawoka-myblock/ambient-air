#![no_std]
use embedded_hal_async::delay::DelayNs;
mod constants;
pub struct Icp20100<I2c, DELAYNS> {
    addr: u8,
    i2c: I2c,
    delay: DELAYNS,
}
#[derive(Debug)]
pub enum Error<E> {
    /// I²C bus error
    I2C(E),
}
impl<E> From<E> for Error<E> {
    fn from(other: E) -> Self {
        Error::I2C(other)
    }
}

impl<I2C: embedded_hal_async::i2c::I2c, Delay: DelayNs> Icp20100<I2C, Delay> {
    pub async fn new(addr: u8, i2c: I2C, delay: Delay) -> Result<Self, Error<I2C::Error>> {
        let mut sensor = Self { addr, i2c, delay };
        sensor.startup().await?;
        sensor.start_measurements().await?;
        Ok(sensor)
    }
    pub async fn get_version(&mut self) -> Result<f32, Error<I2C::Error>> {
        let v = self.read_register(constants::VERSION).await?;
        let major = (v >> 4) & 0x0F;
        let minor = v & 0x0F;
        Ok(major as f32 + (minor as f32 / 10.0))
    }
    async fn startup(&mut self) -> Result<(), Error<I2C::Error>> {
        if !self.check_if_init_is_needed().await? {
            return Ok(());
        }

        self.set_power_mode().await?; // Check
        self.unlock_main_registers().await?; // Check
        self.enable_otp().await?; // Check
        self.toggle_otp_reset().await?; // Check

        self.program_redundant_read().await?;

        // Read OTP calibration values
        let offset = self.read_otp(0xF8).await?; // Check (ish)
        let gain = self.read_otp(0xF9).await?;
        let hfosc = self.read_otp(0xFA).await?;

        self.disable_otp().await?; // check
        self.write_calibration(offset, gain, hfosc).await?; // Check
        self.write_register(constants::MASTER_LOCK, 0x00).await?; // lock main registers
        self.move_to_standby().await?;
        self.mark_boot_done().await?;
        Ok(())
    }

    async fn program_redundant_read(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write_register(constants::OTP_MRA_LSB, 0x04).await?;
        self.write_register(constants::OTP_MRA_MSB, 0x04).await?;
        self.write_register(constants::OTP_MRB_LSB, 0x21).await?;
        self.write_register(constants::OTP_MRB_MSB, 0x20).await?;
        self.write_register(constants::OTP_MR_LSB, 0x10).await?;
        self.write_register(constants::OTP_MR_MSB, 0x80).await?;
        Ok(())
    }
    async fn set_power_mode(&mut self) -> Result<(), Error<I2C::Error>> {
        let mut mode_select = self.read_register(constants::MODE_SELECT).await?;
        mode_select |= 1 << 2;
        self.write_register(constants::MODE_SELECT, mode_select)
            .await?;
        self.delay.delay_ms(4).await;
        Ok(())
    }

    /// Unlocks main registers
    async fn unlock_main_registers(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write_register(constants::MASTER_LOCK, 0x1f).await?;
        Ok(())
    }

    /// Enables OTP and write switch
    async fn enable_otp(&mut self) -> Result<(), Error<I2C::Error>> {
        let mut cfg = self.read_register(constants::OTP_CONFIG1).await?;
        cfg |= 0b11;
        self.write_register(constants::OTP_CONFIG1, cfg).await?; // OTP_ENABLE=1, OTP_WRITE_SWITCH=1
        self.delay.delay_us(10).await;
        Ok(())
    }

    /// Toggles the OTP reset pin
    async fn toggle_otp_reset(&mut self) -> Result<(), Error<I2C::Error>> {
        let mut dbg2 = self.read_register(constants::OTP_DBG2).await?;
        dbg2 |= 1 << 7;
        self.write_register(constants::OTP_DBG2, dbg2).await?;
        self.delay.delay_us(10).await;

        dbg2 &= !(1 << 7);
        self.write_register(constants::OTP_DBG2, dbg2).await?;
        self.delay.delay_us(10).await;
        Ok(())
    }

    /// Performs redundant OTP read
    async fn read_otp(&mut self, address: u8) -> Result<u8, Error<I2C::Error>> {
        // Set the OTP address
        self.write_register(constants::OTP_ADDRESS_REG, address)
            .await?;
        let mut cmd = self.read_register(constants::OTP_COMMAND_REG).await?;
        cmd &= !(0b111 << 4); // clear bits 6:4
        cmd |= 1u8 << 4; // set COMMAND = 1
        self.write_register(constants::OTP_COMMAND_REG, cmd).await?;

        // Wait until OTP_STATUS.BUSY = 0
        loop {
            let status = self.read_register(constants::OTP_STATUS).await?;
            if status & 0x01 == 0 {
                // BUSY bit is LSB
                break;
            }
            self.delay.delay_ms(100).await;
        }

        // Read the value
        let value = self.read_register(constants::OTP_RDATA).await?;
        Ok(value)
    }

    /// Disables OTP and write switch
    async fn disable_otp(&mut self) -> Result<(), Error<I2C::Error>> {
        let mut cfg = self.read_register(constants::OTP_CONFIG1).await?;
        cfg &= !0b11;
        self.write_register(constants::OTP_CONFIG1, cfg).await?; // OTP_ENABLE=0, OTP_WRITE_SWITCH=0
        self.delay.delay_us(10).await;
        Ok(())
    }

    /// Writes calibration values to main registers
    async fn write_calibration(
        &mut self,
        offset: u8,
        gain: u8,
        hfosc: u8,
    ) -> Result<(), Error<I2C::Error>> {
        // TRIM1_MSB.PEFE_OFFSET_TRIM = offset[5:0]
        {
            let mut trim1 = self.read_register(constants::TRIM1_MSB).await?;
            // Clear bits 0–5
            trim1 &= !0b00_111111;
            // Insert offset[5:0]
            trim1 |= offset & 0x3F;
            self.write_register(constants::TRIM1_MSB, trim1).await?;
        }

        // TRIM2_MSB[6:4] = gain[2:0]
        {
            let mut trim2_msb = self.read_register(constants::TRIM2_MSB).await?;
            // Clear bits 4–6
            trim2_msb &= !(0b111 << 4);
            // Insert gain[2:0] into bits 4–6
            trim2_msb |= (gain & 0x07) << 4;
            self.write_register(constants::TRIM2_MSB, trim2_msb).await?;
        }

        // TRIM2_LSB = hfosc
        {
            let mut trim2_lsb = self.read_register(constants::TRIM2_LSB).await?;
            // Clear bits 0–6
            trim2_lsb &= !(0x7F);
            // Insert hfosc[6:0]
            trim2_lsb |= hfosc & 0x7F;
            self.write_register(constants::TRIM2_LSB, trim2_lsb).await?;
        }

        Ok(())
    }

    /// Moves device to standby
    async fn move_to_standby(&mut self) -> Result<(), Error<I2C::Error>> {
        let mut mode_select = self.read_register(constants::MODE_SELECT).await?;
        // Clear POWER_MODE (bit 2)
        mode_select &= !(1 << 2);
        self.write_register(constants::MODE_SELECT, mode_select)
            .await?;
        Ok(())
    }

    /// Marks boot config as done
    async fn mark_boot_done(&mut self) -> Result<(), Error<I2C::Error>> {
        let mut status2 = self.read_register(constants::OTP_STATUS2).await?;
        // Set bit 0 (BOOT_UP_STATUS = 1)
        status2 |= 1 << 0;
        self.write_register(constants::OTP_STATUS2, status2).await?;
        Ok(())
    }
    async fn check_if_init_is_needed(&mut self) -> Result<bool, Error<I2C::Error>> {
        let version = self.read_register(constants::VERSION).await?;
        let otp_status2 = self.read_register(constants::OTP_STATUS2).await?;
        Ok(version == 0x00 && otp_status2 & (1 << 0) == 0)
    }

    pub async fn read_raw_temperature(&mut self) -> Result<u32, Error<I2C::Error>> {
        self.read_u24(constants::TEMP_DATA_0).await
    }
    pub async fn read_raw_pressure(&mut self) -> Result<u32, Error<I2C::Error>> {
        self.read_u24(constants::PRESS_DATA_0).await
    }
    async fn read_fifo_sample(&mut self) -> Result<(i32, i32), Error<I2C::Error>> {
        // Burst read 6 bytes: PRESS0..2, TEMP0..2
        let mut buf = [0u8; 6];
        self.read_burst(constants::PRESS_DATA_0, &mut buf).await?;

        // ---- Pressure (20-bit signed) ----
        let p_raw = ((buf[2] as i32 & 0x0F) << 16) | ((buf[1] as i32) << 8) | (buf[0] as i32);

        // Sign extend 20-bit
        let p_raw = (p_raw << 12) >> 12;

        // ---- Temperature (20-bit signed) ----
        let t_raw = ((buf[5] as i32 & 0x0F) << 16) | ((buf[4] as i32) << 8) | (buf[3] as i32);

        let t_raw = (t_raw << 12) >> 12;

        Ok((p_raw, t_raw))
    }
    async fn start_measurements(&mut self) -> Result<(), Error<I2C::Error>> {
        loop {
            let status = self.read_register(0xCD).await?; // DEVICE_STATUS
            if status & 0x01 != 0 {
                break;
            }
        }

        // MODE_SELECT:
        // bits 7:5 = mode (0..3)
        // bit 3 = MEAS_MODE (continuous)
        // bit 2 = POWER_MODE (active)
        // bits 1:0 = FIFO_READOUT_MODE (00: pressure first)
        let mode_select = (1 << 3)     | // MEAS_MODE = continuous
            (1 << 2); // POWER_MODE = active

        self.write_register(0xC0, mode_select).await?;
        Ok(())
    }

    pub async fn read_pressure_and_temperature(&mut self) -> Result<(f32, f32), Error<I2C::Error>> {
        let (p_raw, t_raw) = self.read_fifo_sample().await?;
        let pressure = self.convert_pressure(p_raw);
        let temperature = self.convert_temperature(t_raw);
        Ok((pressure, temperature))
    }

    pub async fn read_pressure(&mut self) -> Result<f32, Error<I2C::Error>> {
        let (p_raw, _) = self.read_fifo_sample().await?;
        let pressure = self.convert_pressure(p_raw);
        Ok(pressure)
    }
    pub async fn read_temperature(&mut self) -> Result<f32, Error<I2C::Error>> {
        let (_, t_raw) = self.read_fifo_sample().await?;
        let temperature = self.convert_temperature(t_raw);
        Ok(temperature)
    }

    /// Converts raw pressure value to kPa.
    fn convert_pressure(&self, p_raw: i32) -> f32 {
        (p_raw as f32 / (2_i32.pow(17) as f32)) * 40.0 + 70.0
    }

    /// Converts raw temperature value to °C.
    fn convert_temperature(&self, t_raw: i32) -> f32 {
        (t_raw as f32 / (2_i32.pow(18) as f32)) * 65.0 + 25.0
    }
    async fn read_register(&mut self, register: u8) -> Result<u8, Error<I2C::Error>> {
        let mut data = [0; 1];
        self.i2c
            .write_read(self.addr, &[register], &mut data)
            .await?;
        Ok(data[0])
    }
    async fn write_register(&mut self, register: u8, value: u8) -> Result<(), Error<I2C::Error>> {
        let buffer: [u8; 2] = [register, value];
        self.i2c.write(self.addr, &buffer).await?;
        Ok(())
    }
    async fn read_burst(&mut self, start: u8, buffer: &mut [u8]) -> Result<(), Error<I2C::Error>> {
        self.i2c.write_read(self.addr, &[start], buffer).await?;
        let _ = self.read_register(0x00).await?;
        Ok(())
    }
    async fn read_u24(&mut self, base: u8) -> Result<u32, Error<I2C::Error>> {
        let mut buf = [0u8; 3];
        // use the burst read so the address increment feature is used
        self.read_burst(base, &mut buf).await?;
        Ok(((buf[2] as u32) << 16) | ((buf[1] as u32) << 8) | (buf[0] as u32))
    }
}
