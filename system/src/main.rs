use std::{
    collections::HashMap,
    fs::read,
    sync::{Arc, RwLock},
};
use wasmtime::{Module as WasmModule, *};

type Servers = Arc<RwLock<Vec<Server>>>;

struct CallRoute {
    name: String,
    func: String,
    servers: Servers,
}

impl wasmtime::Callable for CallRoute {
    fn call(&self, params: &[Val], results: &mut [Val]) -> Result<(), Trap> {
        let result = self
            .servers
            .read()
            .map_err(|_| Trap::new("Could not read rwlock"))?
            .iter()
            .find_map(|server| server.modules.get(&self.name))
            .ok_or_else(|| Trap::new("No server found with module loaded"))
            .and_then(|module| module.run(&self.func, params).map_err(|err| Trap::new(err)))?;

        for (idx, result) in result.into_iter().enumerate() {
            results[idx] = result.clone();
        }

        Ok(())
    }
}

struct Server {
    modules: HashMap<String, Module>,
}

impl Server {
    fn run(&self, name: &str, func: &str, args: &[Val]) -> Result<Box<[Val]>, String> {
        self.modules.get(name).unwrap().run(func, args)
    }
}

struct Module {
    instance: Instance,
    exports: HashMap<String, usize>,
}

impl Module {
    fn run(&self, func: &str, args: &[Val]) -> Result<Box<[Val]>, String> {
        self.instance
            .exports()
            .get(*self.exports.get(func).ok_or_else(|| String::from("index not found"))?)
            .ok_or_else(|| String::from("entry not found"))
            .and_then(|func| func.func().ok_or_else(|| String::from("Item is not a function")))?
            .borrow()
            .call(args)
            .map_err(|trap| trap.to_string())
    }
}

fn main() {
    let servers = Arc::new(RwLock::new(Vec::new()));

    let server1 = create_server(
        &[
            (
                "https://repository.timot.se/test1",
                "./target/wasm32-unknown-unknown/debug/test1.wasm",
            ),
            (
                "https://repository.timot.se/test2",
                "./target/wasm32-unknown-unknown/debug/test2.wasm",
            ),
        ],
        servers.clone(),
    );

    let server2 = create_server(
        &[(
            "https://repository.timot.se/test2",
            "./target/wasm32-unknown-unknown/debug/test2.wasm",
        )],
        servers.clone(),
    );

    servers.write().unwrap().append(&mut vec![server1, server2]);

    dbg!(servers.read().unwrap()[0]
        .run("https://repository.timot.se/test2", "return_arg", &[111.into()]));

    dbg!(servers.read().unwrap()[1]
        .run("https://repository.timot.se/test2", "return_arg", &[222.into()]));
}

fn create_server(binaries: &[(&str, &str)], servers: Servers) -> Server {
    Server {
        modules: binaries
            .into_iter()
            .map(|(name, path)| {
                let bin = read(path).unwrap();
                let store = Store::default();
                let module = WasmModule::new(&store, &bin).expect("wasm module");

                (name.to_string(), Module {
                    instance: Instance::new(
                        &store,
                        &module,
                        &map_imports(module.imports(), servers.clone()),
                    )
                    .unwrap(),
                    exports: map_exports(module.exports()).collect(),
                })
            })
            .collect(),
    }
}

fn map_imports(imports: &[ImportType], servers: Servers) -> Vec<Extern> {
    imports
        .iter()
        .filter_map(|import| {
            let route = CallRoute {
                servers: servers.clone(),
                name: import.module().into(),
                func: import.name().into(),
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
