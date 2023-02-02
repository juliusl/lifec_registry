macro_rules! cfg_editor {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "editor")]
            $item
        )*
    }
}

macro_rules! cfg_not_editor {
    ($($item:item)*) => {
        $(
            #[cfg(not(feature = "editor"))]
            $item
        )*
    }
}
