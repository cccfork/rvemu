//! The virtio module contains a virtualization standard for a block device.
//! This is the "legacy" virtio interface.
//!
//! The virtio spec:
//! http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-1110002
//! 5.2 Block Device:
//! http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-2020002

use crate::bus::VIRTIO_BASE;
use crate::cpu::Cpu;
use crate::exception::Exception;

/// The interrupt request of virtio.
pub const VIRTIO_IRQ: u64 = 1;

/// The size of `VRingDesc` struct.
const VRING_DESC_SIZE: u64 = 16;
/// The number of virtio descriptors. It must be a power of two.
const QUEUE_SIZE: u64 = 8;
/// The size of a sector.
const SECTOR_SIZE: u64 = 512;

// 4.2.2 MMIO Device Register Layout
// http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-1110002
/// Magic value. Always return 0x74726976 (a Little Endian equivalent of the “virt” string).
pub const VIRTIO_MAGIC: u64 = VIRTIO_BASE + 0x000;
/// Device version number. 1 is legacy.
pub const VIRTIO_VERSION: u64 = VIRTIO_BASE + 0x004;
/// Virtio Subsystem Device ID. 1 is network, 2 is block device.
pub const VIRTIO_DEVICE_ID: u64 = VIRTIO_BASE + 0x008;
/// Virtio Subsystem Vendor ID. Always return 0x554d4551
pub const VIRTIO_VENDOR_ID: u64 = VIRTIO_BASE + 0x00c;
/// Flags representing features the device supports.
pub const VIRTIO_DEVICE_FEATURES: u64 = VIRTIO_BASE + 0x010;
/// Flags representing device features understood and activated by the driver.
pub const VIRTIO_DRIVER_FEATURES: u64 = VIRTIO_BASE + 0x020;
// 4.2.4 Legacy interface
// http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-1210004
/// Guest page size. The driver writes the guest page size in bytes to the register during
/// initialization, before any queues are used. This value should be a power of 2 and is used by
/// the device to calculate the Guest address of the first queue page. Write-only.
pub const VIRTIO_GUEST_PAGE_SIZE: u64 = VIRTIO_BASE + 0x028;
/// Virtual queue index. Writing to this register selects the virtual queue that the following
/// operations on the QueueNumMax, QueueNum, QueueAlign and QueuePFN registers apply to. The index
/// number of the first queue is zero (0x0). Write-only.
pub const VIRTIO_QUEUE_SEL: u64 = VIRTIO_BASE + 0x030;
/// Maximum virtual queue size. Reading from the register returns the maximum size of the queue the
/// device is ready to process or zero (0x0) if the queue is not available. This applies to the
/// queue selected by writing to QueueSel and is allowed only when QueuePFN is set to zero (0x0),
/// so when the queue is not actively used. Read-only. In QEMU, `VIRTIO_COUNT = 8`.
pub const VIRTIO_QUEUE_NUM_MAX: u64 = VIRTIO_BASE + 0x034;
/// Virtual queue size. Queue size is the number of elements in the queue, therefore size of the
/// descriptor table and both available and used rings. Writing to this register notifies the
/// device what size of the queue the driver will use. This applies to the queue selected by
/// writing to QueueSel. Write-only.
pub const VIRTIO_QUEUE_NUM: u64 = VIRTIO_BASE + 0x038;
/// Guest physical page number of the virtual queue. Writing to this register notifies the device
/// about location of the virtual queue in the Guest’s physical address space. This value is the
/// index number of a page starting with the queue Descriptor Table. Value zero (0x0) means
/// physical address zero (0x00000000) and is illegal. When the driver stops using the queue it
/// writes zero (0x0) to this register. Reading from this register returns the currently used page
/// number of the queue, therefore a value other than zero (0x0) means that the queue is in use.
/// Both read and write accesses apply to the queue selected by writing to QueueSel.
pub const VIRTIO_QUEUE_PFN: u64 = VIRTIO_BASE + 0x040;
// 4.2.2 MMIO Device Register Layout
// http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-1110002
/// Queue notifier. Writing a queue index to this register notifies the device that there are new
/// buffers to process in the queue. Write-only.
pub const VIRTIO_QUEUE_NOTIFY: u64 = VIRTIO_BASE + 0x050;
/// Device status. Reading from this register returns the current device status flags. Writing
/// non-zero values to this register sets the status flags, indicating the driver progress. Writing
/// zero (0x0) to this register triggers a device reset.
pub const VIRTIO_STATUS: u64 = VIRTIO_BASE + 0x070;

/// "The descriptor table refers to the buffers the driver is using for the device. addr is a
/// physical address, and the buffers can be chained via next. Each descriptor describes a buffer
/// which is read-only for the device (“device-readable”) or write-only for the device
/// (“device-writable”), but a chain of descriptors can contain both device-readable and
/// device-writable buffers."
///
/// http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-300005
///
/// ```c
/// struct virtq_desc {
///   /* Address (guest-physical). */
///   le64 addr;
///   /* Length. */
///   le32 len;
///
///   /* This marks a buffer as continuing via the next field. */
///   #define VIRTQ_DESC_F_NEXT   1
///   /* This marks a buffer as device write-only (otherwise device read-only). */
///   #define VIRTQ_DESC_F_WRITE     2
///   /* This means the buffer contains a list of buffer descriptors. */
///   #define VIRTQ_DESC_F_INDIRECT   4
///   /* The flags as indicated above. */
///   le16 flags;
///   /* Next field if flags & NEXT */
///   le16 next;
/// };
/// ```
struct VirtqDesc {
    /// 64-bit address.
    addr: u64,
    /// 32-bit length.
    len: u64,
    /// 16-bit flags.
    flags: u64,
    /// 16-bit next.
    next: u64,
}

impl VirtqDesc {
    /// Create a new virtqueue descriptor based on the address that stores the content of the descriptor.
    fn new(cpu: &Cpu, addr: u64) -> Result<Self, Exception> {
        Ok(Self {
            addr: cpu.bus.read64(addr)?,
            len: cpu.bus.read32(addr.wrapping_add(8))?,
            flags: cpu.bus.read16(addr.wrapping_add(12))?,
            next: cpu.bus.read16(addr.wrapping_add(14))?,
        })
    }
}

/// Paravirtualized drivers for IO virtualization.
pub struct Virtio {
    id: u64,
    /// 2.2 Feature Bits
    /// http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-130002
    /// Each virtio device offers all the features it understands.
    /// 0 to 23: Feature bits for the specific device type
    /// 24 to 40: Feature bits reserved for extensions to the queue and
    ///           feature negotiation mechanisms
    /// 41 to 63: Feature bits reserved for future extensions
    driver_features: u32,
    page_size: u32,
    queue_sel: u32,
    queue_num: u32,
    queue_pfn: u32,
    queue_notify: u32,
    /// "The device status field provides a simple low-level indication of the completed steps of
    /// this sequence.
    /// The device MUST initialize device status to 0 upon reset."
    /// https://docs.oasis-open.org/virtio/virtio/v1.1/csprd01/virtio-v1.1-csprd01.html#x1-100001
    status: u32,
    disk: Vec<u8>,
}

impl Virtio {
    /// Create a new virtio object.
    pub fn new() -> Self {
        Self {
            id: 0,
            driver_features: 0,
            page_size: 0,
            queue_sel: 0,
            queue_num: 0,
            queue_pfn: 0,
            queue_notify: 9999, // TODO: what is the correct initial value?
            status: 0,
            disk: Vec::new(),
        }
    }

    /// Return true if an interrupt is pending.
    pub fn is_interrupting(&mut self) -> bool {
        if self.queue_notify != 9999 {
            self.queue_notify = 9999;
            return true;
        }
        false
    }

    /// Set the binary in the virtio disk.
    pub fn set_disk(&mut self, binary: Vec<u8>) {
        self.disk.extend(binary.iter().cloned());
    }

    /// Read 4 bytes from virtio only if the addr is valid. Otherwise, return 0.
    pub fn read(&self, addr: u64) -> u32 {
        match addr {
            VIRTIO_MAGIC => 0x74726976,     // read-only
            VIRTIO_VERSION => 0x1,          // read-only
            VIRTIO_DEVICE_ID => 0x2,        // read-only
            VIRTIO_VENDOR_ID => 0x554d4551, // read-only
            VIRTIO_DEVICE_FEATURES => 0,    // TODO: what should it return?
            VIRTIO_DRIVER_FEATURES => self.driver_features,
            VIRTIO_QUEUE_NUM_MAX => 8,
            VIRTIO_QUEUE_PFN => self.queue_pfn,
            VIRTIO_STATUS => self.status,
            _ => 0,
        }
    }

    /// Write 4 bytes to virtio only if the addr is valid. Otherwise, does nothing.
    pub fn write(&mut self, addr: u64, val: u32) {
        match addr {
            VIRTIO_DEVICE_FEATURES => self.driver_features = val,
            VIRTIO_GUEST_PAGE_SIZE => self.page_size = val,
            VIRTIO_QUEUE_SEL => self.queue_sel = val,
            VIRTIO_QUEUE_NUM => self.queue_num = val,
            VIRTIO_QUEUE_PFN => self.queue_pfn = val,
            VIRTIO_QUEUE_NOTIFY => self.queue_notify = val,
            VIRTIO_STATUS => self.status = val,
            _ => {}
        }
    }

    fn get_new_id(&mut self) -> u64 {
        self.id = self.id.wrapping_add(1);
        self.id
    }

    fn desc_addr(&self) -> u64 {
        self.queue_pfn as u64 * self.page_size as u64
    }

    fn read_disk(&self, addr: u64) -> u64 {
        self.disk[addr as usize] as u64
    }

    fn write_disk(&mut self, addr: u64, value: u64) {
        self.disk[addr as usize] = value as u8
    }

    /// Access the disk via virtio. This is an associated function which takes a `cpu` object to
    /// read and write with a memory directly (DMA).
    pub fn disk_access(cpu: &mut Cpu) -> Result<(), Exception> {
        // https://docs.oasis-open.org/virtio/virtio/v1.1/csprd01/virtio-v1.1-csprd01.html#x1-230005
        // "Each virtqueue can consist of up to 3 parts:
        //     Descriptor Area - used for describing buffers
        //     Driver Area - extra data supplied by driver to the device
        //     Device Area - extra data supplied by device to driver"
        //
        // https://github.com/mit-pdos/xv6-riscv/blob/riscv/kernel/virtio_disk.c#L101-L103
        //     desc = pages -- num * VirtqDesc
        //     avail = pages + 0x40 -- 2 * uint16, then num * uint16
        //     used = pages + 4096 -- 2 * uint16, then num * vRingUsedElem
        //
        // The actual descriptors (16 bytes each).
        let desc_addr = cpu.bus.virtio.desc_addr();
        // A ring of available descriptor heads with free-running index.
        let avail_addr = cpu.bus.virtio.desc_addr() + 0x40;
        // A ring of used descriptor heads with free-running index.
        let used_addr = cpu.bus.virtio.desc_addr() + 4096;

        // 2.4.6 The Virtqueue Available Ring
        // http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-360006
        // struct virtq_avail {
        //   #define VIRTQ_AVAIL_F_NO_INTERRUPT      1
        //   le16 flags;
        //   le16 idx;
        //   le16 ring[ /* Queue Size */ ];
        //   le16 used_event; /* Only if VIRTIO_F_EVENT_IDX */
        // };
        //
        // https://github.com/mit-pdos/xv6-riscv/blob/riscv/kernel/virtio_disk.c#L230-L234
        // "avail[0] is flags
        //  avail[1] tells the device how far to look in avail[2...].
        //  avail[2...] are desc[] indices the device should process.
        //  we only tell device the first index in our chain of descriptors."
        let offset = cpu.bus.read16(avail_addr.wrapping_add(1))?;
        let index = cpu
            .bus
            .read16(avail_addr.wrapping_add(offset % QUEUE_SIZE).wrapping_add(2))?;

        // First descriptor.
        let desc0 = VirtqDesc::new(cpu, desc_addr + VRING_DESC_SIZE * index)?;

        // Second descriptor.
        let desc1 = VirtqDesc::new(cpu, desc_addr + VRING_DESC_SIZE * desc0.next)?;

        // 5.2.6 Device Operation
        // http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-2130006
        // struct virtio_blk_req {
        //   le32 type;
        //   le32 reserved;
        //   le64 sector;
        //   u8 data[][512];
        //   u8 status;
        // };
        let sector = cpu.bus.read64(desc0.addr.wrapping_add(8))?;

        // Write to a device if the second bit of `flags` is set.
        match (desc1.flags & 2) == 0 {
            true => {
                // Read memory data and write it to a disk directly (DMA).
                for i in 0..desc1.len {
                    let data = cpu.bus.read8(desc1.addr + i)?;
                    cpu.bus.virtio.write_disk(sector * SECTOR_SIZE + i, data);
                }
            }
            false => {
                // Read disk data and write it to memory directly (DMA).
                for i in 0..desc1.len {
                    let data = cpu.bus.virtio.read_disk(sector * SECTOR_SIZE + i);
                    cpu.bus.write8(desc1.addr + i, data)?;
                }
            }
        };

        // http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-400008
        // struct virtq_used {
        //   #define VIRTQ_USED_F_NO_NOTIFY  1
        //   le16 flags;
        //   le16 idx;
        //   struct virtq_used_elem ring[ /* Queue Size */];
        //   le16 avail_event; /* Only if VIRTIO_F_EVENT_IDX */
        // };
        let new_id = cpu.bus.virtio.get_new_id();
        cpu.bus.write16(used_addr.wrapping_add(2), new_id % 8)?;
        Ok(())
    }
}
