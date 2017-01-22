extern crate strata_rs;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use std::fs::File;
use std::env;
use std::io::prelude::*;

use strata_rs::{Visit, Visitor, Function};

#[derive(Debug, Default, Serialize, Deserialize)]
struct IndexedFile {
    source: String,
    functions: Vec<strata_rs::Extent>,
    idents: Vec<strata_rs::Extent>,
}

impl Visitor for IndexedFile {
    fn visit_ident(&mut self, ident: &strata_rs::Ident) {
        self.idents.push(ident.extent);
    }

    fn visit_function(&mut self, function: &Function) {
        self.functions.push(function.extent);
    }
}

fn main() {
    let fname = env::args().nth(1).expect("Need filename argument");
    let mut f = File::open(fname).expect("Can't open");
    let mut s = String::new();
    f.read_to_string(&mut s).expect("Can't read");
    let file = strata_rs::parse_rust_file(&s).expect("Failed");
    let mut d = IndexedFile::default();
    d.source = s;

    for i in &file {
        i.visit(&mut d);
    }

    let mut out = std::io::stdout();
    serde_json::ser::to_writer(&mut out, &d).expect("Nope");
}

// Goal:
// What methods take a generic argument that implements a trait `Foo`
