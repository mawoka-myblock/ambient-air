#![no_std]

use embedded_hal_async::i2c::I2c;
mod constants;

#[derive(Debug)]
pub enum Error<E> {
    I2c(E),
    InvalidDevice,
}

impl<E> From<E> for Error<E> {
    fn from(e: E) -> Self {
        Error::I2c(e)
    }
}

pub struct Bq27441<I2C> {
    i2c: I2C,
    addr: u8,
}

impl<I2C: I2c> Bq27441<I2C> {
    pub async fn new(i2c: I2C, addr: u8) -> Result<Self, Error<I2C::Error>> {
        let mut dev = Self { i2c, addr };
        let id = dev.device_type().await?;
        if id != constants::DEVICE_ID {
            return Err(Error::InvalidDevice);
        }
        Ok(dev)
    }

    /* ---------- Public measurements ---------- */

    pub async fn voltage_mv(&mut self) -> Result<u16, Error<I2C::Error>> {
        self.read_word(constants::CMD_VOLTAGE).await
    }

    pub async fn avg_current_ma(&mut self) -> Result<i16, Error<I2C::Error>> {
        Ok(self.read_word(constants::CMD_AVG_CURRENT).await? as i16)
    }

    pub async fn soc_percent(&mut self) -> Result<u16, Error<I2C::Error>> {
        self.read_word(constants::CMD_SOC).await
    }

    pub async fn temperature_c(&mut self) -> Result<f32, Error<I2C::Error>> {
        // Temperature is in 0.1 Kelvin
        let t = self.read_word(constants::CMD_TEMP).await?;
        Ok((t as f32 * 0.1) - 273.15)
    }

    pub async fn state_of_health(&mut self) -> Result<u8, Error<I2C::Error>> {
        let soh = self.read_word(constants::CMD_SOH).await?;
        Ok((soh & 0x00FF) as u8)
    }

    pub async fn average_power_mw(&mut self) -> Result<i16, Error<I2C::Error>> {
        let raw = self.read_word(constants::CMD_AVG_POWER).await?;
        Ok(raw as i16)
    }

    /* ---------- Control ---------- */

    pub async fn device_type(&mut self) -> Result<u16, Error<I2C::Error>> {
        self.read_control(constants::CTRL_DEVICE_TYPE).await
    }

    pub async fn soft_reset(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write_control(constants::CTRL_SOFT_RESET).await
    }

    pub async fn unseal(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write_control(constants::UNSEAL_KEY).await?;
        self.write_control(constants::UNSEAL_KEY).await
    }

    /* ---------- Extended data ---------- */

    pub async fn set_design_capacity(
        &mut self,
        capacity_mah: u16,
    ) -> Result<(), Error<I2C::Error>> {
        let data = capacity_mah.to_be_bytes();
        self.write_extended(82, 10, &data).await
    }

    pub async fn design_capacity_mah(&mut self) -> Result<u16, Error<I2C::Error>> {
        self.read_extended_word(82, 10).await
    }

    /* ---------- Low-level helpers ---------- */

    async fn read_extended_word(
        &mut self,
        class_id: u8,
        offset: u8,
    ) -> Result<u16, Error<I2C::Error>> {
        // Enable block data control
        self.i2c
            .write(self.addr, &[constants::EXT_CONTROL, 0x00])
            .await?;

        // Select data class
        self.i2c
            .write(self.addr, &[constants::EXT_DATACLASS, class_id])
            .await?;

        // Select data block
        self.i2c
            .write(self.addr, &[constants::EXT_DATABLOCK, offset / 32])
            .await?;

        // Read 2 bytes starting at offset
        let mut buf = [0u8; 2];
        self.i2c
            .write_read(
                self.addr,
                &[constants::EXT_BLOCKDATA + (offset % 32)],
                &mut buf,
            )
            .await?;

        Ok(u16::from_be_bytes(buf))
    }

    async fn read_word(&mut self, cmd: u8) -> Result<u16, Error<I2C::Error>> {
        let mut buf = [0u8; 2];
        self.i2c.write_read(self.addr, &[cmd], &mut buf).await?;
        Ok(u16::from_le_bytes(buf))
    }

    async fn write_control(&mut self, cmd: u16) -> Result<(), Error<I2C::Error>> {
        let bytes = cmd.to_le_bytes();
        self.i2c
            .write(self.addr, &[constants::CMD_CONTROL, bytes[0], bytes[1]])
            .await?;
        Ok(())
    }

    async fn read_control(&mut self, cmd: u16) -> Result<u16, Error<I2C::Error>> {
        let bytes = cmd.to_le_bytes();
        self.i2c
            .write(self.addr, &[constants::CMD_CONTROL, bytes[0], bytes[1]])
            .await?;
        self.read_word(constants::CMD_CONTROL).await
    }

    async fn write_extended(
        &mut self,
        class_id: u8,
        offset: u8,
        data: &[u8],
    ) -> Result<(), Error<I2C::Error>> {
        // Enable block access
        self.i2c
            .write(self.addr, &[constants::EXT_CONTROL, 0x00])
            .await?;

        self.i2c
            .write(self.addr, &[constants::EXT_DATACLASS, class_id])
            .await?;

        self.i2c
            .write(self.addr, &[constants::EXT_DATABLOCK, offset / 32])
            .await?;

        for (i, b) in data.iter().enumerate() {
            self.i2c
                .write(
                    self.addr,
                    &[constants::EXT_BLOCKDATA + (offset % 32) + i as u8, *b],
                )
                .await?;
        }

        let checksum = self.compute_checksum().await?;
        self.i2c
            .write(self.addr, &[constants::EXT_CHECKSUM, checksum])
            .await?;
        Ok(())
    }

    async fn compute_checksum(&mut self) -> Result<u8, Error<I2C::Error>> {
        let mut buf = [0u8; 32];
        self.i2c
            .write_read(self.addr, &[constants::EXT_BLOCKDATA], &mut buf)
            .await?;

        let sum: u8 = buf.iter().fold(0u8, |a, b| a.wrapping_add(*b));
        Ok(255u8.wrapping_sub(sum))
    }
}
