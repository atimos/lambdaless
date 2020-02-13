use std::{
    collections::HashMap,
    fs::read,
    sync::{Arc, RwLock},
};
use wasmtime::{Module as WasmModule, *};

type Server = HashMap<String, Module>;

#[derive(Clone, Default)]
struct Servers {
    list: Arc<RwLock<Vec<Server>>>,
}

impl Servers {
    fn add(&self, server: Server) -> Result<(), ()> {
        self.list.write().map(|mut list| list.push(server)).map_err(|_| ())
    }

    fn run(&self, module: &str, function: &str, params: &[Val]) -> Result<Box<[Val]>, Trap> {
        self.list
            .read()
            .map_err(|_| Trap::new("Could not read rwlock"))?
            .iter()
            .find_map(|server| server.get(module))
            .ok_or_else(|| Trap::new("No server found with module loaded"))?
            .run(function, params)
    }
}

struct CallRoute {
    module: String,
    function: String,
    servers: Servers,
}

impl wasmtime::Callable for CallRoute {
    fn call(&self, params: &[Val], results: &mut [Val]) -> Result<(), Trap> {
        println!("Calling {}::{}", self.module, self.function);
        let result = self.servers.run(&self.module, &self.function, params)?;

        for (idx, result) in result.into_iter().enumerate() {
            results[idx] = result.clone();
        }

        Ok(())
    }
}

struct Module {
    instance: Instance,
    exports: HashMap<String, usize>,
}

impl Module {
    fn run(&self, function: &str, args: &[Val]) -> Result<Box<[Val]>, Trap> {
        self.exports
            .get(function)
            .and_then(|idx| self.instance.exports().get(*idx))
            .ok_or_else(|| Trap::new("entry not found"))?
            .func()
            .ok_or_else(|| Trap::new("Item is not a function"))?
            .borrow()
            .call(args)
    }
}

fn main() {
    let servers = Servers::default();

    servers
        .add(create_server(
            &[(
                "https://repository.timot.se/test1",
                "./target/wasm32-unknown-unknown/release/test1.wasm",
            )],
            &servers,
        ))
        .unwrap();

    servers
        .add(create_server(
            &[(
                "https://repository.timot.se/test2",
                "./target/wasm32-unknown-unknown/release/test2.wasm",
            )],
            &servers,
        ))
        .unwrap();

    servers
        .add(create_server(&[("https://repository.timot.se/test3", "./test3/main.wasm")], &servers))
        .unwrap();

    println!("Running https://repository.timot.se/test1::return_double_arg");
    println!(
        "{:?}",
        servers
            .run("https://repository.timot.se/test1", "return_double_arg", &[111.into()])
            .unwrap()
    );
    println!("\nRunning https://repository.timot.se/test2::return_arg");
    println!(
        "{:?}",
        servers.run("https://repository.timot.se/test2", "return_arg", &[222.into()]).unwrap()
    );
    println!("\nRunning https://repository.timot.se/test3::return_arg");
    println!(
        "{:?}",
        servers.run("https://repository.timot.se/test3", "return_arg", &[333.into()]).unwrap()
    );
}

fn create_server(binaries: &[(&str, &str)], servers: &Servers) -> Server {
    binaries
        .into_iter()
        .map(|(name, path)| {
            let bin = read(path).unwrap();
            let store = Store::default();
            let module = WasmModule::new(&store, &bin).expect("wasm module");

            (name.to_string(), Module {
                instance: Instance::new(&store, &module, &map_imports(module.imports(), servers))
                    .unwrap(),
                exports: map_exports(module.exports()).collect(),
            })
        })
        .collect()
}

fn map_imports(imports: &[ImportType], servers: &Servers) -> Vec<Extern> {
    imports
        .iter()
        .filter_map(|import| {
            let route = CallRoute {
                servers: servers.clone(),
                module: import.module().into(),
                function: import.name().into(),
            };
            match import.ty() {
                ExternType::Func(func) => Some(Extern::Func(HostRef::new(wasmtime::Func::new(
                    &Store::default(),
                    func.clone(),
                    std::rc::Rc::new(route),
                )))),
                _ => None,
            }
        })
        .collect()
}

fn map_exports<'a>(exports: &'a [ExportType]) -> impl Iterator<Item = (String, usize)> + 'a {
    exports.iter().enumerate().map(|(index, export)| (export.name().to_owned(), index))
}
