use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf, absolute},
};

use meralus_storage::ResourceStorage;
use mollie::{
    AdtBuilder, GcPtr, VTableBuilder,
    compiler::{Compiler, cranelift::module::ModuleResult, error::CompileError},
    index::Idx,
    typed_ast::{FileModuleLoader, TypedASTContext},
    typing::{AdtRef, Func, ModuleId, Type, TypeContext, TypeRef},
};
use serde::Deserialize;
use tracing::info;

#[allow(dead_code)]
fn add_builtins(context: &mut TypedASTContext) -> TypeRef {
    let color_ty = AdtBuilder::new_struct(&mut context.type_context, "Color")
        .field::<u8>("red")
        .field::<u8>("green")
        .field::<u8>("blue")
        .finish();

    let color_ty = context.type_context.inst_adt(color_ty, &[]);
    let draw_ctx_ty = AdtBuilder::new_struct(&mut context.type_context, "DrawContext").finish();
    let draw_ctx_ty = context.type_context.inst_adt(draw_ctx_ty, &[]);

    let println_str = context.type_context.types.get_or_add(Type::Func(
        Box::new([context.type_context.core_types.string]),
        context.type_context.core_types.void,
    ));

    context.type_context.register_func_in_module(ModuleId::ZERO, Func {
        postfix: false,
        name: "println_str".to_owned(),
        arg_names: Vec::new(),
        ty: println_str,
    });

    let f32 = context.type_context.core_types.f32;
    let void = context.type_context.core_types.void;

    VTableBuilder::new(context, draw_ctx_ty)
        .func("draw_rect", "DrawContext_draw_rect", [draw_ctx_ty, f32, f32, f32, f32, color_ty], void)
        .finish();

    draw_ctx_ty
}

#[derive(Debug, Deserialize)]
pub struct AddonInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct AddonPackage {
    #[serde(rename = "addon")]
    pub info: AddonInfo,
}

#[derive(Debug)]
pub struct Addon {
    pub base: PathBuf,
    pub package: AddonPackage,
    pub main: String,
}

impl Addon {
    pub fn load_all<P: AsRef<Path>>(folder: P) -> Vec<Self> {
        fs::read_dir(folder)
            .map(|entries| {
                entries
                    .flatten()
                    .filter_map(|entry| {
                        let base = entry.path();

                        if base.is_dir() {
                            let package = fs::read(base.join("package.toml")).ok()?;
                            let package = toml::from_slice(&package).ok()?;
                            let main = fs::read_to_string(base.join("src/main.mol")).ok()?;

                            Some(Self { base, package, main })
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

struct DataContext<'a> {
    current_mapping: &'a str,
    storage: &'a mut meralus_storage::ResourceStorage,
}

#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
struct BlockData {
    id: &'static str,
    cull_if_same: bool,
    blocks_light: bool,
    consume_light_level: u8,
    light_level: u8,
    droppable: bool,
    collidable: bool,
    selectable: bool,
}

impl meralus_storage::Block for BlockData {
    fn id(&self) -> &'static str {
        self.id
    }

    fn cull_if_same(&self) -> bool {
        self.cull_if_same
    }

    fn blocks_light(&self) -> bool {
        self.blocks_light
    }

    fn consume_light_level(&self) -> u8 {
        self.consume_light_level
    }

    fn light_level(&self) -> u8 {
        self.light_level
    }

    fn droppable(&self) -> bool {
        self.droppable
    }

    fn collidable(&self) -> bool {
        self.collidable
    }

    fn selectable(&self) -> bool {
        self.selectable
    }
}

#[allow(clippy::needless_pass_by_value)]
impl DataContext<'_> {
    fn register_block(&mut self, data: GcPtr<BlockData>) {
        // println!("[{}] Registering {}", "INFO/BlockLoader  ".bright_green(),
        // data.id.bright_blue().bold());

        self.storage.register_block(self.current_mapping, *data);
    }
}

impl BlockData {
    fn register(context: &mut TypeContext) -> AdtRef {
        AdtBuilder::new_struct(context, "Block")
            .field::<&str>("id")
            .field_default::<bool>("cull_if_same", false)
            .field_default::<bool>("blocks_light", true)
            .field_default::<u8>("consume_light_level", 0)
            .field_default::<u8>("light_level", 0)
            .field_default::<bool>("droppable", true)
            .field_default::<bool>("collidable", true)
            .field_default::<bool>("selectable", true)
            .finish()
    }
}

impl DataContext<'_> {
    fn register(context: &mut TypeContext) -> AdtRef {
        AdtBuilder::new_struct(context, "DataContext").non_gc_collectable().finish()
    }
}

pub type Mappings = HashMap<String, PathBuf>;

pub struct AddonManager {
    addons: Vec<Addon>,
    compiler: Compiler<FileModuleLoader>,
}

#[allow(clippy::missing_errors_doc, clippy::missing_panics_doc, clippy::result_large_err)]
impl AddonManager {
    pub fn new<P: AsRef<Path>>(folder: P) -> ModuleResult<Self> {
        Ok(Self {
            addons: Addon::load_all(folder),
            compiler: Compiler::with_symbols(
                FileModuleLoader {
                    current_dir: PathBuf::from("/"),
                },
                [("DataContext_register_block", DataContext::register_block as *const u8)],
            )?,
        })
    }

    pub fn insert_mappings(&self, storage: &mut ResourceStorage) -> io::Result<()> {
        for addon in &self.addons {
            storage
                .mappings
                .insert(addon.package.info.name.clone(), absolute(addon.base.join("resources"))?);
        }

        Ok(())
    }

    pub fn execute(&mut self, storage: &mut ResourceStorage) -> ModuleResult<()> {
        let mut func_compiler = self.compiler.start_compiling();
        let ptr_type = func_compiler.compiler.ptr_type();
        let mut context = DataContext {
            current_mapping: "game",
            storage,
        };

        let block_ty = BlockData::register(&mut func_compiler.type_context.type_context);
        let block_ty = func_compiler.type_context.type_context.inst_adt(block_ty, &[]);
        let data_ctx_ty = DataContext::register(&mut func_compiler.type_context.type_context);
        let data_ctx_ty = func_compiler.type_context.type_context.inst_adt(data_ctx_ty, &[]);
        let void = func_compiler.type_context.type_context.core_types.void;

        VTableBuilder::new(func_compiler.type_context, data_ctx_ty)
            .func("register_block", "DataContext_register_block", [data_ctx_ty, block_ty], void)
            .finish();

        for addon in &self.addons {
            info!(target: "addons", "Loading {} v{}", addon.package.info.name, addon.package.info.version);

            context.current_mapping = &addon.package.info.name;
            func_compiler.module_loader.current_dir = addon.base.join("src");

            let name = format!("{}_<main>", addon.package.info.name);

            if let Err(CompileError::Type(errors)) = func_compiler.compile(name.as_str(), [("data", ptr_type, data_ctx_ty)], None, &addon.main) {
                let file = addon.base.join("src/main.mol").display().to_string();
                let file = file.as_str();

                for error in errors {
                    let span = (file, error.span.start..error.span.end);
                    let mut report = ariadne::Report::build(ariadne::ReportKind::Error, span.clone()).with_config(ariadne::Config::new().with_compact(true));

                    error.value.add_to_report(span, &mut report, &func_compiler.type_context.type_context);

                    report.finish().print((file, ariadne::Source::from(&addon.main))).unwrap();
                }
            }

            unsafe { func_compiler.compiler.get_func::<fn(&mut DataContext)>(name).unwrap()(&mut context) };
        }

        Ok(())
    }
}
