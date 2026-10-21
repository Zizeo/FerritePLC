use std::env;
use std::path::PathBuf;

use ferrite_gui::{GuiError, run_gui};

fn main() -> Result<(), GuiError> {
    let path_arg = env::args().nth(1).map(PathBuf::from);
    run_gui(path_arg)
}
