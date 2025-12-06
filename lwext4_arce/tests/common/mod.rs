use std::fs::File;
use std::io::{Read, Write, Seek, SeekFrom};
use lwext4_arce::{BlockDevice, Ext4Result, Ext4Error};

pub struct FileBlockDevice {
    file: File,
}

impl FileBlockDevice {
    pub fn open(path: &str) -> std::io::Result<Self> {
        Ok(Self {
            file: File::options().read(true).write(true).open(path)?
        })
    }
}

impl BlockDevice for FileBlockDevice {
    fn read_blocks(&mut self, block_id: u64, buf: &mut [u8]) -> Ext4Result<usize> {
        self.file.seek(SeekFrom::Start(block_id * 512))
            .map_err(|_| Ext4Error::new(libc::EIO, "seek failed"))?;
        self.file.read(buf)
            .map_err(|_| Ext4Error::new(libc::EIO, "read failed"))
    }

    fn write_blocks(&mut self, block_id: u64, buf: &[u8]) -> Ext4Result<usize> {
        self.file.seek(SeekFrom::Start(block_id * 512))
            .map_err(|_| Ext4Error::new(libc::EIO, "seek failed"))?;
        self.file.write(buf)
            .map_err(|_| Ext4Error::new(libc::EIO, "write failed"))
    }

    fn num_blocks(&self) -> Ext4Result<u64> {
        let size = self.file.metadata()
            .map_err(|_| Ext4Error::new(libc::EIO, "metadata failed"))?
            .len();
        Ok(size / 512)
    }
}
