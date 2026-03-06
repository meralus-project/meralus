use std::{
    fs,
    path::{Path, PathBuf},
};

use mollie::{
    AdtBuilder, GcPtr, VTableBuilder,
    compiler::FuncCompiler,
    index::Idx,
    typing::{AdtKind, FieldType, FuncArg, TypeInfo, TypeInfoRef},
};
use serde::Deserialize;

// type Program = fn(*mut RenderContext, *const GameMetadata);

fn add_builtins(func_compiler: &mut FuncCompiler) -> TypeInfoRef {
    let color_ty = func_compiler.checker.register_adt(
        AdtBuilder::new_struct("Color")
            .field::<u8>("red")
            .field::<u8>("green")
            .field::<u8>("blue")
            .finish(),
    );

    let (color_info, _) = func_compiler.checker.instantiate_adt(color_ty, &[]);
    let draw_ctx_ty = func_compiler.checker.register_adt(AdtBuilder::new_struct("DrawContext").finish());
    let (draw_ctx_info, _) = func_compiler.checker.instantiate_adt(draw_ctx_ty, &[]);

    let println_str = func_compiler.checker.solver.add_info(
        TypeInfo::Func(
            Box::new([FuncArg::Regular(func_compiler.checker.core_types.string)]),
            func_compiler.checker.core_types.void,
        ),
        None,
    );

    func_compiler.checker.solver.add_var("context", draw_ctx_info);
    func_compiler.checker.solver.add_var("println_str", println_str);

    VTableBuilder::new(FieldType::Adt(draw_ctx_ty, AdtKind::Struct, Box::new([])))
        .func(
            "draw_rect",
            "DrawContext_draw_rect",
            [
                draw_ctx_info,
                func_compiler.checker.core_types.float,
                func_compiler.checker.core_types.float,
                func_compiler.checker.core_types.float,
                func_compiler.checker.core_types.float,
                color_info,
            ],
            func_compiler.checker.core_types.void,
        )
        .finish(&mut func_compiler.checker);

    // func_compiler.compiler.var_ty(
    //     "println",
    //     TypeVariant::function([TypeVariant::one_of([TypeVariant::int64(),
    // TypeVariant::usize()])], TypeVariant::void()), );

    // func_compiler
    //     .compiler
    //     .var_ty("println_str", TypeVariant::function([TypeVariant::string()],
    // TypeVariant::void())); func_compiler
    //     .compiler
    //     .var_ty("println_bool",
    // TypeVariant::function([TypeVariant::boolean()], TypeVariant::void()));
    // func_compiler
    //     .compiler
    //     .var_ty("println_addr", TypeVariant::function([TypeVariant::any()],
    // TypeVariant::void())); func_compiler
    //     .compiler
    //     .var_ty("get_type_idx", TypeVariant::function([TypeVariant::any()],
    // TypeVariant::usize())); func_compiler
    //     .compiler
    //     .var_ty("get_size", TypeVariant::function([TypeVariant::any()],
    // TypeVariant::usize()));

    // let draw_ctx_ty = TypeVariant::structure::<String, Type, _>([]);
    // let metadata_ty =
    // TypeVariant::structure_ir(func_compiler.compiler.jit.module.isa(), [
    //     ("window_width", TypeVariant::float()),
    //     ("window_height", TypeVariant::float()),
    // ]);

    // let color_ty =
    // TypeVariant::structure_ir(func_compiler.compiler.jit.module.isa(), [
    //     ("red", TypeVariant::uint8()),
    //     ("green", TypeVariant::uint8()),
    //     ("blue", TypeVariant::uint8()),
    // ]);

    // let object_fit_ty = TypeVariant::enumeration(["Stretch", "Cover"]);

    // let corner_radius_ty =
    // TypeVariant::structure_ir(func_compiler.compiler.jit.module.isa(), [
    //     ("top_left", TypeVariant::float()),
    //     ("top_right", TypeVariant::float()),
    //     ("bottom_left", TypeVariant::float()),
    //     ("bottom_right", TypeVariant::float()),
    // ]);

    // let draw_ctx_type_idx = func_compiler.compiler.add_type("DrawContext",
    // draw_ctx_ty.clone());

    // func_compiler.compiler.add_type("Color", color_ty.clone());
    // func_compiler.compiler.add_type("CornerRadius",
    // corner_radius_ty.clone()); func_compiler.compiler.add_type("
    // ObjectFit", object_fit_ty.clone());

    // let metadata_ty_idx = func_compiler.compiler.add_type("GameMetadata",
    // metadata_ty.clone());

    // func_compiler.compiler.var("metadata", metadata_ty_idx);
    // func_compiler.compiler.var("context", draw_ctx_type_idx);

    // let draw_rect = func_compiler
    //     .add_native_fn(
    //         "DrawContext_draw_rect",
    //         Some(draw_ctx_ty.clone()),
    //         [
    //             TypeVariant::float(),
    //             TypeVariant::float(),
    //             TypeVariant::float(),
    //             TypeVariant::float(),
    //             color_ty.clone(),
    //         ],
    //         TypeVariant::void(),
    //     )
    //     .unwrap_or_else(|e| panic!("failed to add DrawContext_draw_rect:
    // {e}"));

    // let draw_round_rect = func_compiler
    //     .add_native_fn(
    //         "DrawContext_draw_rrect",
    //         Some(draw_ctx_ty.clone()),
    //         [
    //             TypeVariant::float(),
    //             TypeVariant::float(),
    //             TypeVariant::float(),
    //             TypeVariant::float(),
    //             corner_radius_ty.clone(),
    //             color_ty.clone(),
    //         ],
    //         TypeVariant::void(),
    //     )
    //     .unwrap_or_else(|e| panic!("failed to add DrawContext_draw_rrect:
    // {e}"));

    // let draw_image = func_compiler
    //     .add_native_fn(
    //         "DrawContext_draw_image",
    //         Some(draw_ctx_ty.clone()),
    //         [
    //             TypeVariant::float(),
    //             TypeVariant::float(),
    //             TypeVariant::float(),
    //             TypeVariant::float(),
    //             TypeVariant::string(),
    //             object_fit_ty,
    //         ],
    //         TypeVariant::void(),
    //     )
    //     .unwrap_or_else(|e| panic!("failed to add DrawContext_draw_image:
    // {e}"));

    // let draw_round_image = func_compiler
    //     .add_native_fn(
    //         "DrawContext_draw_round_image",
    //         Some(draw_ctx_ty.clone()),
    //         [
    //             TypeVariant::float(),
    //             TypeVariant::float(),
    //             TypeVariant::float(),
    //             TypeVariant::float(),
    //             corner_radius_ty,
    //             TypeVariant::string(),
    //         ],
    //         TypeVariant::void(),
    //     )
    //     .unwrap_or_else(|e| panic!("failed to add
    // DrawContext_draw_round_image: {e}"));

    // let draw_text = func_compiler
    //     .add_native_fn(
    //         "DrawContext_draw_text",
    //         Some(draw_ctx_ty),
    //         [
    //             TypeVariant::float(),
    //             TypeVariant::float(),
    //             TypeVariant::string(),
    //             TypeVariant::string(),
    //             TypeVariant::float(),
    //             color_ty,
    //         ],
    //         TypeVariant::void(),
    //     )
    //     .unwrap_or_else(|e| panic!("failed to add DrawContext_draw_text:
    // {e}"));

    // func_compiler
    //     .create_fallback_vtable(draw_ctx_type_idx, [
    //         ("draw_rect", draw_rect),
    //         ("draw_rrect", draw_round_rect),
    //         ("draw_image", draw_image),
    //         ("draw_round_image", draw_round_image),
    //         ("draw_text", draw_text),
    //     ])
    //     .unwrap_or_else(|e| panic!("failed to create fallback vtable for
    // DrawContext: {e}"));
    draw_ctx_info
}

#[derive(Debug, Deserialize)]
struct AddonInfo {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct AddonPackage {
    #[serde(rename = "addon")]
    info: AddonInfo,
}

#[derive(Debug)]
struct Addon {
    base: PathBuf,
    package: AddonPackage,
    main: String,
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
    resources: &'a mut meralus_storage::ResourceStorage,
}

#[derive(Debug, Clone, Copy)]
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

impl DataContext<'_> {
    fn register_block(&mut self, data: GcPtr<BlockData>) {
        // println!("[{}] Registering {}", "INFO/BlockLoader  ".bright_green(),
        // data.id.bright_blue().bold());

        self.resources.register_block(self.current_mapping, *data);
    }
}

// let mut compiler = Compiler::with_symbols(vec![
// ("DataContext_register_block", DataContext::register_block as *const u8),
// ("WorldData_send_chat_message", WorldData::send_chat_message as *const u8),
// ("EventManager_on_tick", EventManager::on_tick as *const u8),
// ("EventManager_on_world_start", EventManager::on_world_start as *const u8),
// ])
// .unwrap();
//
// let mut func_compiler = compiler.start_compiling();
// let ptr_type = func_compiler.compiler.ptr_type();
//
// let block_ty = func_compiler.checker.register_adt(
// AdtBuilder::new_struct("Block")
// .field::<&str>("id")
// .field_default::<bool>("cull_if_same", false)
// .field_default::<bool>("blocks_light", true)
// .field_default::<u8>("consume_light_level", 0)
// .field_default::<u8>("light_level", 0)
// .field_default::<bool>("droppable", true)
// .field_default::<bool>("collidable", true)
// .field_default::<bool>("selectable", true)
// .finish(),
// );
//
// let data_ctx_ty = func_compiler
// .checker
// .register_adt(AdtBuilder::new_struct("DataContext").non_gc_collectable().
// finish());
//
// let world_data_ty = func_compiler
// .checker
// .register_adt(AdtBuilder::new_struct("WorldData").non_gc_collectable().
// finish());
//
// let event_mgr_ty = func_compiler
// .checker
// .register_adt(AdtBuilder::new_struct("EventManager").non_gc_collectable().
// finish());
//
// let (data_ctx_info, _) = func_compiler.checker.instantiate_adt(data_ctx_ty,
// &[]); let (event_mgr_info, _) =
// func_compiler.checker.instantiate_adt(event_mgr_ty, &[]);
// let (world_data_info, _) =
// func_compiler.checker.instantiate_adt(world_data_ty, &[]);
// let (block_ty_info, _) = func_compiler.checker.instantiate_adt(block_ty,
// &[]); let println_str_info = func_compiler.checker.solver.add_info(
// TypeInfo::Func(
// Box::new([FuncArg::Regular(func_compiler.checker.core_types.string)]),
// func_compiler.checker.core_types.void,
// ),
// None,
// );
//
// func_compiler.checker.solver.add_var("data", data_ctx_info);
// func_compiler.checker.solver.add_var("events", event_mgr_info);
// func_compiler.checker.solver.add_var("println_str", println_str_info);
//
// VTableBuilder::new(FieldType::Adt(data_ctx_ty, AdtKind::Struct,
// Box::new([]))) .func(
// "register_block",
// "DataContext_register_block",
// [data_ctx_info, block_ty_info],
// func_compiler.checker.core_types.void,
// )
// .finish(&mut func_compiler.checker);
//
// let tick_func_info = func_compiler
// .checker
// .solver
// .add_info(TypeInfo::Func(Box::new([]),
// func_compiler.checker.core_types.void), None);
//
// let world_start_func_info = func_compiler.checker.solver.add_info(
// TypeInfo::Func(Box::new([FuncArg::Regular(world_data_info)]),
// func_compiler.checker.core_types.void), None,
// );
//
// VTableBuilder::new(FieldType::Adt(world_data_ty, AdtKind::Struct,
// Box::new([]))) .func(
// "send_chat_message",
// "WorldData_send_chat_message",
// [world_data_info, func_compiler.checker.core_types.string],
// func_compiler.checker.core_types.void,
// )
// .finish(&mut func_compiler.checker);
//
// VTableBuilder::new(FieldType::Adt(event_mgr_ty, AdtKind::Struct,
// Box::new([]))) .func(
// "on_tick",
// "EventManager_on_tick",
// [event_mgr_info, tick_func_info],
// func_compiler.checker.core_types.void,
// )
// .func(
// "on_world_start",
// "EventManager_on_world_start",
// [event_mgr_info, world_start_func_info],
// func_compiler.checker.core_types.void,
// )
// .finish(&mut func_compiler.checker);
//
// let addons = Addon::load_all("./addons");
//
// for addon in &addons {
// resources
// .mappings
// .insert(addon.package.info.name.clone(),
// absolute(addon.base.join("resources")).unwrap()); }
//
// let mut events = EventManager { handlers: Vec::new() };
// let mut context = DataContext {
// current_mapping: "game",
// resources: &mut resources,
// };
//
// sender.new_stage("Addons loading", addons.len())?;
//
// for addon in &addons {
// println!(
// "[{}] Loading {}",
// "INFO/AddonLoader  ".bright_green(),
// format!("{} v{}", addon.package.info.name,
// addon.package.info.version).bright_blue().bold() );
//
// context.current_mapping = &addon.package.info.name;
//
// let name = format!("{}_<main>", addon.package.info.name);
//
// if let Err(CompileError::Type(errors)) = func_compiler.compile(
// &name,
// vec![("data".into(), ptr_type, data_ctx_info), ("events".into(), ptr_type,
// event_mgr_info)], vec![],
// &addon.main,
// ) {
// for error in errors {
// let mut report = ariadne::Report::build(ariadne::ReportKind::Error,
// ("ui.mol", error.span.start..error.span.end))
// .with_config(ariadne::Config::new().with_compact(true));
//
// let label = ariadne::Label::new(("ui.mol",
// error.span.start..error.span.end)).with_color(ariadne::Color::Cyan);
//
// match error.value {
// mollie::typed_ast::TypeError::ExpectedFunction { found } => {
// report.set_message("expected `function`");
// report.add_label(match found {
// NotFunction::Type(found) => {
// label.with_message(format!("found value of type `{}`",
// func_compiler.checker.short_display_of_type(found, None))) }
// NotFunction::Adt(adt_ref) => {
// label.with_message(format!("found `{}`", match
// func_compiler.checker.adt_types[adt_ref].kind { AdtKind::Struct => "struct",
// AdtKind::Component => "component",
// AdtKind::Enum => "enum",
// }))
// }
// NotFunction::Trait(_) => label.with_message("found `trait`"),
// NotFunction::Primitive(primitive_type) => label.with_message(format!(
// "found primitive type `{}`",
// func_compiler
// .checker
// .short_display_of_type(func_compiler.checker.core_types.
// cast_primitive(primitive_type), None) )),
// });
// }
// mollie::typed_ast::TypeError::ExpectedConstructable { found } => {
// report.set_message("expected `struct`, `enum` or `component`");
// report.add_label(label.with_message(format!("found `{}`", match found {
// mollie::typed_ast::NonConstructable::Trait => "trait",
// mollie::typed_ast::NonConstructable::Function => "function",
// mollie::typed_ast::NonConstructable::Generic => "generic",
// mollie::typed_ast::NonConstructable::Module => "module",
// })));
// }
// mollie::typed_ast::TypeError::ExpectedArray { found } => {
// report.set_message("expected `array`");
// report.add_label(label.with_message(format!("found `{}`",
// func_compiler.checker.short_display_of_type(found, None)))); }
// mollie::typed_ast::TypeError::ExpectedModule { found } => {
// report.set_message("expected `module`");
// report.add_label(label.with_message(format!("found `{}`", match found {
// mollie::typed_ast::NotModule::Trait => "trait",
// mollie::typed_ast::NotModule::Function => "function",
// mollie::typed_ast::NotModule::Generic => "generic",
// mollie::typed_ast::NotModule::Adt => "adt",
// })));
// }
// mollie::typed_ast::TypeError::NoField { ty, name } => {
// report.set_message("no field");
// report.add_label(label.with_message(format!(
// "`{}` doesn't have field called `{name}`",
// func_compiler.checker.short_display_of_type(ty, None)
// )));
// }
// mollie::typed_ast::TypeError::NonIndexable { ty, name } => {
// report.set_message("non-indexable value");
// report.add_label(label.with_message(format!(
// "`{}` can't have fields and be indexed by `{name}`",
// func_compiler.checker.short_display_of_type(ty, None)
// )));
// }
// mollie::typed_ast::TypeError::TypeNotFound { name, module } => {
// report.set_message("type not found");
// report.add_label(if module == ModuleId::ZERO {
// label.with_message(format!("there's no type called `{name}`"))
// } else {
// label.with_message(format!(
// "there's no type called `{name}` in `{}`",
// func_compiler.checker.display_of_module(module)
// ))
// });
// }
// mollie::typed_ast::TypeError::InvalidPostfixFunction { reasons } => {
// report.set_message("invalid postfix function definition");
//
// for reason in reasons {
// let label = ariadne::Label::new(("ui.mol",
// reason.span.start..reason.span.end)).with_color(ariadne::Color::Cyan);
//
// report.add_label(match reason.value {
// mollie::typed_ast::PostfixRequirement::NoGenerics =>
// label.with_message("generics are not allowed in postfix context"),
// mollie::typed_ast::PostfixRequirement::OneArgument => label.with_message("one
// argument is expected"),
// mollie::typed_ast::PostfixRequirement::OnlyOneArgument =>
// label.with_message("multiple arguments are not allowed"),
// mollie::typed_ast::PostfixRequirement::ArgumentType => {
// label.with_message("argument type must be either an (unsigned) integer or a
// float") }
// });
// }
// }
// mollie::typed_ast::TypeError::NoVariable { name } => {
// report.set_message(format!("there's no variable called `{name}`"));
// report.add_label(label.with_message("tried to access here"));
// }
// mollie::typed_ast::TypeError::NoFunction { name, postfix } => {
// if postfix {
// report.set_message(format!("there's no postfix function called `{name}`"));
// report.add_label(label.with_message("tried to use here"));
// } else {
// report.set_message(format!("there's no function called `{name}`"));
// report.add_label(label.with_message("tried to call here"));
// }
// }
// mollie::typed_ast::TypeError::InvalidTypePathSegment { reason, module } => {
// report.set_message("invalid type-path segment");
// report.add_label(if module == ModuleId::ZERO {
// match reason {
// InvalidTypePathSegmentReason::Variable(_) => label.with_message("expected
// `type`, found `variable`"), InvalidTypePathSegmentReason::Primitive(_) =>
// label.with_message("primitive types are not allowed in this context"), }
// } else {
// match reason {
// InvalidTypePathSegmentReason::Variable(_) => unreachable!(),
// InvalidTypePathSegmentReason::Primitive(_) => label.with_message("primitive
// types are not allowed in type paths"), }
// });
// }
// mollie::typed_ast::TypeError::Unification(error) => match error {
// mollie::typing::TypeUnificationError::TypeMismatch(expected, got) => {
// report.set_message("type mismatch");
// report.add_label(label.with_message(format!(
// "expected `{}`, found `{}",
// func_compiler.checker.short_display_of_type(expected, None),
// func_compiler.checker.short_display_of_type(got, None)
// )));
// }
// mollie::typing::TypeUnificationError::UnimplementedTrait(trait_ref,
// type_info_ref) => { report.set_message("unimplemented trait");
// report.add_label(label.with_message(format!(
// "expected `{}` to implement `{}",
// func_compiler.checker.short_display_of_type(type_info_ref, None),
// func_compiler.checker.traits[trait_ref].name
// )));
// }
// mollie::typing::TypeUnificationError::ArraySizeMismatch(..) => todo!(),
// mollie::typing::TypeUnificationError::UnknownType(_type_info_ref) => todo!(),
// },
// mollie::typed_ast::TypeError::NotPostfix { name } => {
// report.set_message(format!("`{name}` can't be used in postfix context"));
// report.add_label(label.with_message("tried to use here"));
// }
// mollie::typed_ast::TypeError::ModuleIsNotValue => {
// report.set_message("expected `value`");
// report.add_label(label.with_message("found `module`"));
// }
// mollie::typed_ast::TypeError::NonConstantEvaluable => {
// report.set_message("expression can't be evaluated at compile-time");
// report.add_label(label.with_message("this expression"));
// }
// mollie::typed_ast::TypeError::Parse(parse_error) => {
// report.set_message(parse_error);
// }
// }
//
// report.finish().print(("ui.mol",
// ariadne::Source::from(&addon.main))).unwrap();
//
// panic!();
// }
// }
//
// unsafe { func_compiler.compiler.get_func::<fn(&mut DataContext, &mut
// EventManager)>(name).unwrap()(&mut context, &mut events) };
//
// sender.complete_task()?;
// }
//
// events.trigger(Event::Tick);
// events.trigger(Event::Tick);
// events.trigger(Event::Tick);
// events.trigger(Event::Tick);
// events.trigger(Event::Tick);
