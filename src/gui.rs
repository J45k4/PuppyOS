
pub enum Element {
    Box {
        children: Vec<Element>,
    },
    Text {
        text: String,
    },
    Image {
        src: String
    }
}