use std::process::Command;

fn main() {
    let output = Command::new("source").arg("./build_files/torb_env/bin/activate").output();

    match output {
        Ok(res) => {
            dbg!(res);
        }
        Err(e) => {
            dbg!(e);
        }
    }

}
