// enum Option<T> {
//     Some { value: T },
//     None
// }

// trait Iterable<T> {
//     fn iter(self) -> T;
// }

// trait Iterator<T> {
//     fn next(self) -> Option<T>;
// }

// impl<T> T[] {
//     fn size(self) -> uint_size {
//         get_size(self)
//     }
// }

// struct ArrayIter<T> {
//     value: T[],
//    index: uint_size
}

impl<T> trait Iterator<T> for ArrayIter<T> {
    fn next(self) -> Option<T> {
        if self.index == get_size(self.value) {
            Option::None<T>
        } else {
            const returned = self.value[self.index];

            self.index = self.index + 1uint_size;

            Option::Some<T> {
                value: returned
            }
        }
    }
}

impl<T> trait Iterable<ArrayIter<T>> for T[] {
    fn iter(self) -> ArrayIter<T> {
        ArrayIter {
            value: self,
            index: 0
        }
    }
}

struct Constraints {
    min_width: float = 0.0,
    max_width: float = 0.0,
    min_height: float = 0.0,
    max_height: float = 0.0,
}

trait Placeable {
    fn get_width(self) -> float;
    fn get_height(self) -> float;

    fn get_measured_width(self) -> float;
    fn get_measured_height(self) -> float;

    fn place(self, x: float, y: float);
    fn render(self, ctx: DrawContext);
}

trait Measurable {
    fn measure(self, constraints: Constraints) -> Placeable;
}

declare Rectangle {
    x: float,
    y: float,
    width: float,
    height: float,
    corner_radius: float = 0.0,
    color: Color
}

declare Text {
    x: float,
    y: float,
    font: string,
    font_size: float,
    text: string,
    color: Color
}

impl trait Placeable for Text {
    fn get_width(self) -> float {
        self.font_size
    }

    fn get_height(self) -> float {
        self.font_size
    }

    fn get_measured_width(self) -> float {
        self.font_size
    }

    fn get_measured_height(self) -> float {
        self.font_size
    }

    fn place(self, x: float, y: float) {
        self.x = x;
        self.y = y;
    }

    fn render(self, ctx: DrawContext) {
        ctx.draw_text(self.x, self.y, self.font, self.text, self.font_size, self.color);
    }
}

impl trait Placeable for Rectangle {
    fn get_width(self) -> float {
        self.width
    }

    fn get_height(self) -> float {
        self.height
    }

    fn get_measured_width(self) -> float {
        self.width
    }

    fn get_measured_height(self) -> float {
        self.height
    }

    fn place(self, x: float, y: float) {
        self.x = x;
        self.y = y;
    }

    fn render(self, ctx: DrawContext) {
        
            ctx.draw_rrect(self.x, self.y, self.width, self.height, CornerRadius { top_left: self.corner_radius, top_right: self.corner_radius, bottom_left: self.corner_radius, bottom_right: self.corner_radius }, self.color);
        
    }
}

declare Container {
    vertical: boolean,
    spacing: float = 0.0,
    width: float = 0.0,
    height: float = 0.0,
    children: Placeable[]
}

impl trait Placeable for Container {
    fn get_width(self) -> float {
        self.width
    }

    fn get_height(self) -> float {
        self.height
    }

    fn get_measured_width(self) -> float {
        self.width
    }

    fn get_measured_height(self) -> float {
        self.height
    }

    fn place(self, x: float, y: float) {
        let offset = if self.vertical == true {
            y
        } else {
            x
        };

        let iterator = ArrayIter<Placeable> {
            value: self.children,
            index: 0
        };

        while iterator.next() is Option::Some { value } {
            if self.vertical == true {
                value.place(x, offset);
            } else {
                value.place(offset, y);
            }

            if value is Rectangle rect {
                offset = offset + if self.vertical == true { rect.height + self.spacing } else { rect.width + self.spacing };
            } else if value is Text text {
                offset = offset + if self.vertical == true { text.font_size + self.spacing } else { text.font_size + self.spacing };
            }
        }
    }

    fn render(self, ctx: DrawContext) {
        let iterator = ArrayIter<Placeable> {
            value: self.children,
            index: 0
        };

        while iterator.next() is Option::Some { value } {
            value.render(ctx);
        }
    }
}

const contained = Container {
    vertical: true,
    spacing: 8.0,
    
    Rectangle {
        x: 0.0,
        y: 0.0,
        width: 128.0,
        height: 256.0,
        color: Color { red: 101, green: 235, blue: 134 }
    }

    Text {
        x: 0.0,
        y: 0.0,
        font: "default",
        font_size: 36.0,
        text: "Hello, World!",
        color: Color { red: 255, green: 0, blue: 0 }
    }
    
    Rectangle {
        x: 24.0,
        y: 0.0,
        width: metadata.window_width,
        height: 128.0,
        corner_radius: 24.0,
        color: Color { red: 101, green: 235, blue: 134 }
    }
};

// contained.place(0.0, 0.0);
// contained.render(context);

// context.draw_rect(4.0, 4.0, 96.0, 36.0, Color { red: 101, green: 235, blue: 134 });
// context.draw_text(4.0, 48.0, "default", "ебать это работает?", 36.0, Color { red: 101, green: 235, blue: 134 });
// context.draw_image(0.0, 0.0, metadata.window_width, metadata.window_height, "/home/aiving/Загрузки/photo_5271633797185668025_y.jpg", ObjectFit::Cover);
