pub(crate) trait Idx {
    fn wrapping_idx(&self, x: isize, y: isize) -> usize;
    fn try_idx(&self, x: isize, y: isize) -> Option<usize>;
}

pub(crate) type Shape = (isize, isize);

impl Idx for Shape {
    fn wrapping_idx(&self, x: isize, y: isize) -> usize {
        let (width, height) = self;
        ((x + width) % width + (y + height) % height * width) as usize
    }

    fn try_idx(&self, x: isize, y: isize) -> Option<usize> {
        let (width, height) = self;
        if x < 0 || *width <= x || y < 0 || *height <= y {
            None
        } else {
            Some((x + y * width) as usize)
        }
    }
}

pub(crate) type Size = (usize, usize);

impl Idx for Size {
    fn wrapping_idx(&self, x: isize, y: isize) -> usize {
        let (width, height) = self;
        (x as usize + width) % width + (y as usize + height) % height * width
    }

    fn try_idx(&self, x: isize, y: isize) -> Option<usize> {
        let (width, height) = self;
        if x < 0 || *width as isize <= x || y < 0 || *height as isize <= y {
            None
        } else {
            Some(x as usize + y as usize * width)
        }
    }
}
