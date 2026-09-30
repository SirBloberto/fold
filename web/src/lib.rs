use std::cell::RefCell;
use std::ops::Range;

use fold::{Picture, Value};

struct Slot {
    picture: Picture,
    rgba: Vec<u8>,
    packed: Vec<u8>,
    rows: Range<usize>,
}

impl Slot {
    fn update(&mut self, time: f32) {
        self.picture.frame(time);
        let width = self.picture.width();
        let rgba = self.rgba.as_chunks_mut::<4>().0;
        let pixels = self.picture.pixels();
        for area in self.picture.changed() {
            for row in area.top..area.top + area.height {
                let span = row * width + area.left..row * width + area.left + area.width;
                for (rgba, pixel) in rgba[span.clone()].iter_mut().zip(&pixels[span]) {
                    let [_, r, g, b] = pixel.to_be_bytes();
                    *rgba = [r, g, b, 255];
                }
            }
        }
    }

    fn pack(&mut self) {
        let width = self.picture.width();
        self.packed.clear();
        for area in self.picture.changed() {
            for number in [area.left, area.top, area.width, area.height] {
                self.packed.extend((number as u32).to_le_bytes());
            }
            for row in area.top..area.top + area.height {
                let start = (row * width + area.left) * 4;
                self.packed.extend(&self.rgba[start..start + area.width * 4]);
            }
        }
    }
}

thread_local! {
    static INBOX: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static MESSAGE: RefCell<String> = const { RefCell::new(String::new()) };
    static SLOTS: RefCell<Vec<Option<Slot>>> = const { RefCell::new(Vec::new()) };
}

fn with_slot<T>(id: usize, act: impl FnOnce(&mut Slot) -> T) -> Option<T> {
    SLOTS.with_borrow_mut(|slots| slots.get_mut(id)?.as_mut().map(act))
}

#[unsafe(no_mangle)]
pub extern "C" fn inbox(length: usize) -> *mut u8 {
    INBOX.with_borrow_mut(|inbox| {
        inbox.clear();
        inbox.resize(length, 0);
        inbox.as_mut_ptr()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn load() -> i32 {
    let loaded = INBOX.with_borrow(|inbox| match std::str::from_utf8(inbox) {
        Ok(source) => fold::load(source),
        Err(_) => Err("the file is not valid UTF-8".into()),
    });
    match loaded {
        Ok(picture) => {
            let rgba = vec![0; picture.width() * picture.height() * 4];
            let rows = 0..picture.height();
            SLOTS.with_borrow_mut(|slots| {
                slots.push(Some(Slot {
                    picture,
                    rgba,
                    packed: Vec::new(),
                    rows,
                }));
                slots.len() as i32 - 1
            })
        }
        Err(message) => {
            MESSAGE.set(message);
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn message() -> *const u8 {
    MESSAGE.with_borrow(|message| message.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn message_length() -> usize {
    MESSAGE.with_borrow(|message| message.len())
}

#[unsafe(no_mangle)]
pub extern "C" fn inputs(id: usize) -> usize {
    let line = |input: fold::Input| match input.default {
        Value::Num(n) => {
            let (lo, hi) = input.range.unwrap_or((n, n));
            format!("{}\tnum\t{lo}\t{hi}\t{n}", input.name)
        }
        Value::Colour([r, g, b, a]) => format!("{}\tcolour\t{r}\t{g}\t{b}\t{a}", input.name),
    };
    let lines = with_slot(id, |slot| slot.picture.inputs().into_iter().map(line).collect())
        .unwrap_or_else(Vec::new);
    MESSAGE.set(lines.join("\n"));
    lines.len()
}

fn set(id: usize, value: Value) -> i32 {
    let name = INBOX.with_borrow(|inbox| String::from_utf8_lossy(inbox).into_owned());
    match with_slot(id, |slot| slot.picture.set(&name, value)) {
        Some(Ok(())) => 0,
        Some(Err(message)) => {
            MESSAGE.set(message);
            -1
        }
        None => -1,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn set_num(id: usize, value: f32) -> i32 {
    set(id, Value::Num(value))
}

#[unsafe(no_mangle)]
pub extern "C" fn set_colour(id: usize, r: f32, g: f32, b: f32, a: f32) -> i32 {
    set(id, Value::Colour([r, g, b, a]))
}

#[unsafe(no_mangle)]
pub extern "C" fn width(id: usize) -> usize {
    with_slot(id, |slot| slot.picture.width()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn height(id: usize) -> usize {
    with_slot(id, |slot| slot.picture.height()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn resize(id: usize, width: usize) {
    with_slot(id, |slot| {
        slot.picture.resize(width);
        let (width, height) = (slot.picture.width(), slot.picture.height());
        slot.rgba.resize(width * height * 4, 0);
        slot.rows = 0..height;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn band(id: usize, top: usize, rows: usize) {
    with_slot(id, |slot| {
        let height = slot.picture.height();
        let top = top.min(height);
        slot.rows = top..(top + rows).min(height);
        slot.picture.band(slot.rows.clone());
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn frame(id: usize, time: f32) -> *const u8 {
    with_slot(id, |slot| {
        slot.update(time);
        slot.rgba[slot.rows.start * slot.picture.width() * 4..].as_ptr()
    })
    .unwrap_or(std::ptr::null())
}

#[unsafe(no_mangle)]
pub extern "C" fn changes(id: usize, time: f32) -> usize {
    with_slot(id, |slot| {
        slot.update(time);
        slot.pack();
        slot.packed.len()
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn packed(id: usize) -> *const u8 {
    with_slot(id, |slot| slot.packed.as_ptr()).unwrap_or(std::ptr::null())
}

#[unsafe(no_mangle)]
pub extern "C" fn unload(id: usize) {
    SLOTS.with_borrow_mut(|slots| {
        if let Some(slot) = slots.get_mut(id) {
            *slot = None;
        }
    });
}
