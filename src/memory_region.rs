#[repr(C)]
pub struct MemoryRegion {
    addr: usize,
    size: usize,
}


impl MemoryRegion {
    pub fn new(addr: usize, size: usize) -> MemoryRegion
    {
        MemoryRegion {
            addr: addr,
            size: size,
        }
    }


    pub fn addr(&self) -> usize
    {
        self.addr
    }


    pub fn size(&self) -> usize
    {
        self.size
    }
}


#[cfg(test)]
mod tests {
    use core::mem;
    use super::*;

    #[test]
    fn test_all() {
        const size: usize = 4096;
        let x: &[usize; size] = unsafe { mem::zeroed() };
        let addr = (x as (*const _)) as usize;

        let r = MemoryRegion::new(addr, size);
        assert_eq!(addr, r.addr());
        assert_eq!(size, r.size());
    }
}
