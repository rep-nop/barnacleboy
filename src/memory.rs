use cpu::LRError;

pub trait MemoryInterface {
    type Word;
    type Index;
    type Error;

    fn read(&self, address: Self::Index) -> Result<Self::Word, Self::Error>;
    fn write(&mut self, address: Self::Index, data: Self::Word) -> Result<(), Self::Error>;
}

pub struct MemoryRegion {
    data: Box<[u8]>,
    start: usize,
    end: usize,
}

impl MemoryRegion {
    pub fn new(start: usize, end: usize) -> MemoryRegion {
        assert!(start > end);

        MemoryRegion {
            data: vec![0; end - start + 1].into_boxed_slice(),
            start,
            end,
        }
    }

    pub fn get(&self, index: u16) -> Result<u8, LRError> {
        self.data
            .get(index as usize - self.start)
            .map(|&b| b)
            .ok_or(LRError::InvalidMemoryRead(index))
    }

    pub fn set(&mut self, index: u16, data: u8) -> Result<(), LRError> {
        if let Some(byte) = self.data.get_mut(index as usize - self.start) {
            *byte = data;
            Ok(())
        } else {
            Err(LRError::InvalidMemoryWrite(index))
        }
    }
}

/// Areas of the memory map that are static between different memory controllers.
pub struct SharedMemoryRegions {
    video_ram: MemoryRegion,
    work_ram_0: MemoryRegion,
    work_ram_1: MemoryRegion,
    oam: MemoryRegion,
    io: MemoryRegion,
    high_ram: MemoryRegion,
}

impl SharedMemoryRegions {
    pub fn new() -> SharedMemoryRegions {
        SharedMemoryRegions {
            video_ram: MemoryRegion::new(0x8000, 0x9FFF),
            work_ram_0: MemoryRegion::new(0xC000, 0xCFFF),
            work_ram_1: MemoryRegion::new(0xD000, 0xDFFF),
            oam: MemoryRegion::new(0xFE00, 0xFE9F),
            io: MemoryRegion::new(0xFF00, 0xFF7F),
            high_ram: MemoryRegion::new(0xFF80, 0xFFFF),
        }
    }

    pub fn get(&self, address: u16) -> Result<u8, LRError> {
        let addr = address as usize;
        if addr > self.video_ram.start && addr <= self.video_ram.end {
            self.video_ram.get(address)
        } else if addr > self.work_ram_0.start && addr <= self.work_ram_0.end {
            self.work_ram_0.get(address)
        } else if addr > self.work_ram_1.start && addr <= self.work_ram_1.end {
            self.work_ram_1.get(address)
        } else if addr > self.oam.start && addr <= self.oam.end {
            self.oam.get(address)
        } else if addr > self.io.start && addr <= self.io.end {
            self.io.get(address)
        } else if addr > self.high_ram.start && addr <= self.high_ram.end {
            self.high_ram.get(address)
        } else {
            Err(LRError::InvalidMemoryRead(address))
        }
    }

    pub fn set(&mut self, address: u16, data: u8) -> Result<(), LRError> {
        let addr = address as usize;
        if addr > self.video_ram.start && addr <= self.video_ram.end {
            self.video_ram.set(address, data)
        } else if addr > self.work_ram_0.start && addr <= self.work_ram_0.end {
            self.work_ram_0.set(address, data)
        } else if addr > self.work_ram_1.start && addr <= self.work_ram_1.end {
            self.work_ram_1.set(address, data)
        } else if addr > self.oam.start && addr <= self.oam.end {
            self.oam.set(address, data)
        } else if addr > self.io.start && addr <= self.io.end {
            self.io.set(address, data)
        } else if addr > self.high_ram.start && addr <= self.high_ram.end {
            self.high_ram.set(address, data)
        } else {
            Err(LRError::InvalidMemoryWrite(address))
        }
    }
}

/// ROM only. Contains 32KiB of ROM mapped to 0x0000 to 0x7FFF, with an optional
/// 8KiB of RAM mapped at 0xA000-0xBFFF.
pub struct RomOnly {
    rom: MemoryRegion,
    ram: Option<MemoryRegion>,
    shared_mem: SharedMemoryRegions,
}

impl RomOnly {
    pub fn blank() -> RomOnly {
        RomOnly {
            rom: MemoryRegion::new(0x0000, 0x7FFF),
            ram: None,
            shared_mem: SharedMemoryRegions::new(),
        }
    }
}

impl MemoryInterface for RomOnly {
    type Word = u8;
    type Index = u16;
    type Error = LRError;

    fn read(&self, address: Self::Index) -> Result<Self::Word, Self::Error> {
        match address {
            0x0000...0x7FFF => self.rom.get(address),
            0xA000...0xBFFF => if let Some(ram) = &self.ram {
                ram.get(address)
            } else {
                Err(LRError::InvalidMemoryRead(address))
            },
            _ => self.shared_mem.get(address),
        }
    }

    fn write(&mut self, address: Self::Index, data: Self::Word) -> Result<(), Self::Error> {
        match address {
            0x0000...0x7FFF => Err(LRError::InvalidMemoryWrite(address)),
            0xA000...0xBFFF => if let Some(ram) = &mut self.ram {
                ram.set(address, data)
            } else {
                Err(LRError::InvalidMemoryWrite(address))
            },
            _ => self.shared_mem.set(address, data),
        }
    }
}

enum ModeSelect {
    Rom,
    Ram,
}

pub struct Mbc1 {
    rom_banks: Vec<MemoryRegion>,
    rom_bank_select: usize,
    ram_banks: Vec<MemoryRegion>,
    ram_bank_select: usize,
    shared_mem: SharedMemoryRegions,
    ram_enabled: bool,
    mode: ModeSelect,
}

impl Mbc1 {
    pub fn blank(num_rom_banks: usize, ram_size: usize, num_ram_banks: usize) -> Mbc1 {
        Mbc1 {
            rom_banks: {
                let mut v = Vec::with_capacity(num_rom_banks);
                v.push(MemoryRegion::new(0x0000, 0x3FFF));

                for _ in 1..num_rom_banks {
                    v.push(MemoryRegion::new(0x4000, 0x7FFF));
                }

                v
            },
            rom_bank_select: 1,
            ram_banks: vec![MemoryRegion::new(0xA000, 0xBFFF); num_ram_banks],
            ram_bank_select: 0,
            shared_mem: SharedMemoryRegions::new(),
            ram_enabled: false,
            mode: ModeSelect::Rom,
        }
    }
}

// TODO: Bank number in error?
impl MemoryInterface for Mbc1 {
    type Word = u8;
    type Index = u16;
    type Error = LRError;

    fn read(&self, address: Self::Index) -> Result<Self::Word, Self::Error> {
        match address {
            0x0000...0x3FFF => self.rom_banks[0].get(address),
            0x4000...0x7FFF => self.rom_banks[self.rom_bank_select].get(address),
            0xA000...0xBFFF => if self.ram_enabled {
                self.ram_banks[self.ram_bank_select].get(address)
            } else {
                Err(LRError::InvalidMemoryRead(address))
            },
            _ => self.shared_mem.get(address),
        }
    }

    fn write(&mut self, address: Self::Index, data: Self::Word) -> Result<(), Self::Error> {
        match address {
            0x0000...0x1FFF => if data & 0b1111 == 0xA {
                self.ram_enabled = true;
                Ok(())
            } else {
                self.ram_enabled = false;
                Ok(())
            },
            0x2000...0x3FFF => {
                let select = (data & 0b11111) as usize;

                if select == 0 {
                    select = 1;
                }

                self.rom_bank_select = (self.rom_bank_select & !0b11111) | select;

                Ok(())
            }
            0x4000...0x5FFF => {
                let select = (data & 0b11) as usize;

                match self.mode {
                    ModeSelect::Rom => {
                        self.rom_bank_select = (self.rom_bank_select & !0b1100000) | (select << 5);

                        if self.rom_bank_select == 0x20 || self.rom_bank_select == 0x40
                            || self.rom_bank_select == 0x60
                        {
                            self.rom_bank_select += 1;
                        }
                    }
                    ModeSelect::Ram => {
                        self.ram_bank_select = select;
                    }
                }

                Ok(())
            }
            0x6000...0x7FFF => {
                if data & 0b1 == 0 {
                    self.mode = ModeSelect::Rom;
                } else {
                    self.mode = ModeSelect::Ram;
                }

                Ok(())
            }
            0xA000...0xBFFF => {
                if self.ram_enabled {
                    self.ram_banks[self.ram_bank_select].set(address, data)
                } else {
                    Err(LRError::InvalidMemoryWrite(address))
                }
            }
            _ => self.shared_mem.set(address, data),
        }
    }
}

pub struct Mbc2 {
    rom_banks: Vec<MemoryRegion>,
    rom_bank_select: usize,
    internal_ram: MemoryRegion,
    shared_mem: SharedMemoryRegions,
    ram_enabled: bool,
}

impl Mbc2 {
    pub fn blank(num_rom_banks: usize, ram_size: usize, num_ram_banks: usize) -> Mbc2 {
        Mbc2 {
            rom_banks: {
                let mut v = Vec::with_capacity(num_rom_banks);
                v.push(MemoryRegion::new(0x0000, 0x3FFF));

                for _ in 1..num_rom_banks {
                    v.push(MemoryRegion::new(0x4000, 0x7FFF));
                }

                v
            },
            rom_bank_select: 1,
            internal_ram: MemoryRegion::new(0xA000, 0xA1FF),
            shared_mem: SharedMemoryRegions::new(),
            ram_enabled: false,
        }
    }
}

impl MemoryInterface for Mbc2 {
    type Word = u8;
    type Index = u16;
    type Error = LRError;

    fn read(&self, address: Self::Index) -> Result<Self::Word, Self::Error> {
        match address {
            0x0000...0x3FFF => self.rom_banks[0].get(address),
            0x4000...0x7FFF => self.rom_banks[self.rom_bank_select].get(address),
            0xA000...0xA1FF => if self.ram_enabled {
                self.internal_ram.get(address)
            } else {
                Err(LRError::InvalidMemoryRead(address))
            },
            _ => self.shared_mem.get(address),
        }
    }

    fn write(&mut self, address: Self::Index, data: Self::Word) -> Result<(), Self::Error> {
        match address {
            0x0000...0x1FFF => if (data & 0x01FF) >> 8 == 0 {
                if data & 0b1111 == 0xA {
                    self.ram_enabled = true;
                    Ok(())
                } else {
                    self.ram_enabled = false;
                    Ok(())
                }
            } else {
                Ok(())
            },
            0x2000...0x3FFF => if (data & 0x01FF) >> 8 == 1 {
                let select = (data & 0b1111) as usize;

                if select == 0 {
                    select = 1;
                }

                self.rom_bank_select = (self.rom_bank_select & !0b1111) | select;

                Ok(())
            } else {
                Ok(())
            },
            0xA000...0xA1FF => if self.ram_enabled {
                self.internal_ram.set(address, data & 0x0F)
            } else {
                Err(LRError::InvalidMemoryWrite(address))
            },
            _ => self.shared_mem.set(address, data),
        }
    }
}
