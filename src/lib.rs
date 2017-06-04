#![no_std]
mod memory_region;

use memory_region::MemoryRegion;
use core::{mem, ptr};


trait allocator {
    fn malloc(usize) -> usize;
    fn free(usize);
}


struct MemoryManager<'a> {
    memory_regions: &'a [MemoryRegion],
    head: &'a BoundaryTag,
}


impl<'a> MemoryManager<'a> {
    fn new(mems: &'a [MemoryRegion]) -> MemoryManager
    {
        assert!(mems.len() != 0);

        // TODO: use all memory regions.
        let addr = mems[0].addr();
        let size = mems[0].size();

        MemoryManager {
            memory_regions: mems,
            head: BoundaryTag::from_memory(addr, size),
        }
    }
}


#[repr(C)]
#[derive(Debug)]
struct BoundaryTag {
    is_alloc: bool,
    is_sentinel: bool,
    size: usize,
}


impl<'a> BoundaryTag {
    fn from_memory(addr: usize, size: usize) -> &'a mut BoundaryTag
    {
        let tag = unsafe { &mut *(addr as *mut BoundaryTag) };

        tag.is_alloc = false;
        tag.is_sentinel = true;
        tag.size = size;

        tag
    }
}


#[cfg(test)]
mod tests {
    use core::mem;
    use super::MemoryManager;
    use super::MemoryRegion;
    use super::BoundaryTag;

    #[test]
    fn test_all()
    {
        const SIZE: usize = 4096;
        let x: &[u8; SIZE] = unsafe { mem::zeroed() };
        let addr = (x as *const _) as usize;

        let _ = MemoryRegion::new(addr, SIZE);
    }

    #[test]
    #[should_panic]
    fn test_memory_manager_panic()
    {
        let slice: &[MemoryRegion] = &[];
        let _ = MemoryManager::new(slice);
    }

    #[test]
    fn test_boundary_tag_from_memory()
    {
        const SIZE: usize = 4096;
        let ref mut x: [u8; SIZE] = unsafe { mem::zeroed() };

        assert_eq!(SIZE, mem::size_of_val(x));

        let addr = (x as *const _) as usize;

        let tag = BoundaryTag::from_memory(addr, SIZE);
        assert_eq!((tag as *const _) as usize, addr);
        assert_eq!(tag.size, SIZE);
        assert_eq!(tag.is_alloc, false);
        assert_eq!(tag.is_sentinel, true);
    }
}
