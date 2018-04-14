use drivers::mmio;
use mem::*;

#[repr(C, align(4096))]
pub struct SectionTable
{
    ttbl: [usize; 4096]
}

#[derive(Clone, Copy)]
pub enum RegionAttribute
{
    /// Strongly-ordered (shareable is ignored)
    StronglyOrdered = 0b000,

    /// Shareable device (shareable is ignored)
    Device = 0b001,

    /// Outer and Inner Non-cacheable
    NonCacheable = 0b100,

    /// Outer and Inner Write-Through, no Write-Allocate
    WriteThrough = 0b010,

    /// Outer and Inner Write-Back, no Write-Allocate
    WriteBack = 0b011,

    /// Outer and Inner Write-Back, Write-Allocate
    WriteAllocate = 0b111,
}

#[derive(Clone, Copy)]
pub enum RegionAccess
{
    /// All accesses generate Permission faults
    Forbidden = 0b000,

    /// Access only at PL1
    KernelOnly = 0b001,

    /// Writes at PL0 generate Permission faults
    ReadOnlyKernelWrite = 0b010,

    /// Full access
    Full = 0b011,

    /// Read-only, only at PL1
    KernelReadOnly = 0b101,

    /// Read-only at any privilege level
    ReadOnly = 0b111,
}

pub struct RegionFlags
{
    execute: bool,
    global: bool,
    shareable: bool,
    access: RegionAccess,
    attributes: RegionAttribute,
}

impl SectionTable
{
    pub const fn new() -> SectionTable
    {
        SectionTable { ttbl: [0; 4096] }
    }

    pub fn unregister(&mut self, vaddr_base: usize)
    {
        self.ttbl[vaddr_base] = 0;
    }

    pub fn register_section(&mut self, vaddr_base: usize, paddr_base: usize,
                            flags: &RegionFlags, kernel_execute: bool)
    {
        let mut entry = (paddr_base << 20) | (1 << 1);
        if !flags.execute { entry |= 1 << 4; }
        if !flags.global { entry |= 1 << 17; }
        if flags.shareable { entry |= 1 << 16; }
        entry |= (flags.access as usize & 0b011) << 10;
        entry |= (flags.access as usize & 0b100) << (15-2);
        entry |= (flags.attributes as usize & 0b00011) << 2;
        entry |= (flags.attributes as usize & 0b11100) << (12-2);
        if !kernel_execute { entry |= 1 << 0; }

        self.ttbl[vaddr_base] = entry;
    }

    pub fn register_page_table(&mut self, vaddr_base: usize,
                               page_table: *const PageTable,
                               kernel_execute: bool)
    {
        let mut entry = page_table as usize | (1 << 0);
        if !kernel_execute { entry |= 1 << 2; }
        self.ttbl[vaddr_base] = entry;
    }
}

#[repr(C, align(1024))]
pub struct PageTable
{
    ttbl: [usize; 256]
}

impl PageTable
{
    pub const fn new() -> PageTable
    {
        PageTable { ttbl: [0; 256] }
    }

    pub fn unregister(&mut self, vaddr_offset: usize)
    {
        self.ttbl[vaddr_offset] = 0;
    }

    pub fn register_page(&mut self, vaddr_offset: usize, paddr_base: usize,
                         flags: &RegionFlags)
    {
        let mut entry = (paddr_base << 12) | (1 << 1);
        if !flags.execute { entry |= 1 << 0; }
        if !flags.global { entry |= 1 << 11; }
        if flags.shareable { entry |= 1 << 10; }
        entry |= (flags.access as usize & 0b011) << 4;
        entry |= (flags.access as usize & 0b100) << (9-2);
        entry |= (flags.attributes as usize & 0b011) << 2;
        entry |= (flags.attributes as usize & 0b100) << (6-2);

        self.ttbl[vaddr_offset] = entry;
    }
}

unsafe fn setup_ttbr0(translation_table: *const SectionTable)
{
    // Disable MMU, cache and branch prediction
    let mut sctlr : u32;
    asm!("mrc p15, 0, $0, c1, c0, 0" : "=r"(sctlr));
    sctlr &= !(1 << 29 | 1 << 28 | 1 << 12 | 1 << 11 | 1 << 2 | 1 << 0);
    asm!("mcr p15, 0, $0, c1, c0, 0" :: "r"(sctlr) :: "volatile");

    // Clean cache and TLB
    asm!("mcr p15, 0, $0, c7, c1, 0
          mcr p15, 0, $0, c8, c3, 0"
          :: "r"(0) :: "volatile");

    mmio::sync_barrier();

    // Setup TTBCR, TTBR0 and DACR
    let ttbr0 = translation_table as usize | 0b1001010;
    asm!("mcr p15, 0, $0, c2, c0, 2
          mcr p15, 0, $1, c2, c0, 0
          mcr p15, 0, $2, c3, c0, 0"
          :: "r"(0), "r"(ttbr0), "r"(1) :: "volatile");

    // Setup SCTLR (enable Instruction cache, branch prediction (Z),
    // SWp instruction, Cache, Alignment check, Mmu)
    sctlr |= 1 << 12 | 1 << 11 | 1 << 10 | 1 << 2 | 1 << 1 | 1 << 0;
    asm!("mcr p15, 0, $0, c1, c0, 0" :: "r"(sctlr) :: "volatile");

    mmio::sync_barrier();
}

static mut KERNEL_SECTION_TABLE: SectionTable = SectionTable::new();
static mut KERNEL_PAGE_TABLE: PageTable = PageTable::new();

linker_symbol!
{
    static __text;
    static __rodata;
    static __data;
}

/**
 * Create the kernel identity mapping.
 * All addresses below 0x4000_0000 are mapped to themselves.
 * They are accessible by kernel only.
 * All other addresses are unavailable.
 * This function also enables caches. As a consequence,
 * looping code is way faster after this function has been called.
 */
pub fn init()
{
    let sections;
    let pages;
    unsafe
    {
        sections = &mut KERNEL_SECTION_TABLE;
        pages = &mut KERNEL_PAGE_TABLE;
    }

    let kernel_text_flags = RegionFlags { execute: true, global: true,
        shareable: false, access: RegionAccess::KernelReadOnly,
        attributes: RegionAttribute::WriteAllocate };

    let kernel_rodata_flags = RegionFlags { execute: false, global: true,
        shareable: false, access: RegionAccess::KernelReadOnly,
        attributes: RegionAttribute::WriteAllocate };

    let kernel_data_flags = RegionFlags { execute: false, global: true,
        shareable: true, access: RegionAccess::KernelOnly,
        attributes: RegionAttribute::WriteAllocate };

    let fst_text_page = linker_symbol!(__text) / PAGE_SIZE;
    let fst_rodata_page = linker_symbol!(__rodata) / PAGE_SIZE;
    let fst_data_page = linker_symbol!(__data) / PAGE_SIZE;

    // .text.start and ATAGS
    pages.register_page(0, 0, &kernel_text_flags);

    // Kernel stack
    for i in 1 .. fst_text_page
    {
        pages.register_page(i, i, &kernel_data_flags);
    }

    // .text
    for i in fst_text_page .. fst_rodata_page
    {
        pages.register_page(i, i, &kernel_text_flags);
    }

    // .rodata
    for i in fst_rodata_page .. fst_data_page
    {
        pages.register_page(i, i, &kernel_rodata_flags);
    }

    // .data, .bss and after
    for i in fst_data_page .. PAGE_BY_SECTION
    {
        pages.register_page(i, i, &kernel_data_flags);
    }

    // Use pages above
    sections.register_page_table(0, pages, true);

    // Standard data sections
    for i in 1 .. mmio::PERIPHERAL_BASE / SECTION_SIZE
    {
        sections.register_section(i, i, &kernel_data_flags, false);
    }

    // Peripheral sections
    let periph_flags = RegionFlags { execute: false, global: true,
        shareable: true, access: RegionAccess::KernelOnly,
        attributes: RegionAttribute::Device };
    for i in mmio::PERIPHERAL_BASE / SECTION_SIZE .. NUM_SECTION_MAX
    {
        sections.register_section(i, i, &periph_flags, false);
    }

    unsafe
    {
        setup_ttbr0(&KERNEL_SECTION_TABLE as *const SectionTable);
    }
}