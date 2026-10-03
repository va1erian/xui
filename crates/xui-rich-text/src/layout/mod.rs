#![forbid(unsafe_code)]

//! The flow engine: breaks a [`Document`](crate::Document)'s paragraphs into
//! lines around floating images, through a backend's `TextShaper`.
