# aitios-cli
Provides a command line interface for weathering with aitios
based on the gammaton tracing algorithm proposed by Chen and
other researchers.

See `tests/examples` for usage examples.

    grace:aitios-cli krachzack$ aitios-cli --help
    aitios 0.1
    krachzack <hello@phstadler.com>
    Procedural weathering simulation on the command line with aitios

    USAGE:
        aitios-cli [FLAGS] <SIMULATION_SPEC_FILE>

    FLAGS:
        -h, --help       Prints help information
        -l, --log        Specifies a file in which to log output
        -V, --version    Prints version information
        -v, --verbose    Activates verbose output

    ARGS:
        <SIMULATION_SPEC_FILE>    Sets the path to the simulation config YAML file

## What aitios is
Aitios is a tool to simulate aging of materials in virtual scenes. It does this by
running a simulation of aging-inducing particles that interact with the materials
in the input scene. The simulation generates texture maps indicating the weathering
degrees as substance concentrations over the surface. These can either be used directly as masks in
your favorite rendering application, or be further used by aitios to apply blemishes
to the base materials of objects using a blending technique.

## How to use
Simulations in aitios are described in YAML specification files. There are three types of these spec files.

### Simulation Spec
Contains meta information about the simulation, describes
input scene and global simulation parameters.

Materials in the input scene are associated with surfel
specs in this configuration file.

It provides a list of ton source specs, configuring the
emission of gammatons.

A list of effects describes how the simulated material
distribution textures are used to synthesize output
textures and scenes.

    # Meta information
    name: Park Scene
    description: "A single buddha in the center gets bombarded with rain from the sky, making it rust, everything not made of bronze is concrete."

    # Input scene
    scene: "tests/assets/buddha.obj"

    # After iteration 0, which runs the effects on the
    # unmodified input scene as a reference, the simulation
    # will execute 30 tracing/rule/effect cycles known as
    # iterations.
    # That is, there will be 31 iterations, with iterations
    # 1-30 performing actual tracing.
    iterations: 30

    # There will be one gammaton source described in the
    # Ton Source Spec located at the specified path.
    sources:
      - "rain.yml"

    # Maps MTL material names against Surfel Description specs
    surfels_by_material:
      # Every material named bronze will be configured
      # as specified in iron.yml
      bronze: "iron.yml"
      # catchall, every not otherwise specified material will
      # be specified in concrete.yml
      _: "concrete.yml"

    # Describes how the final concentration of materials will
    # be used for texture synthesis.
    effects:
      # Output textures indicating the concentration of
      # substances on all entities in the scene and optionally
      # generate a fresh OBJ file on each iteration where the
      # textures are applied to the objects in the scene.
      - density:
        # Common density texture size for all entities
        width: 4096
        height: 4096
        # Pixels to draw over the edges of triangles when they
        # have no neighbor in UV space. This ensures no texture
        # seam artifacts if correctly configured.
        island_bleed: 3
        # Patterns for generated PNG/OBJ/MTL files.
        # The {expressions} will be automatically replaced
        # during generation to avoid name conflicts.
        tex_pattern: "{datetime}/iteration-{iteration}/{id}-{entity}-{substance}.png"
        obj_pattern: "{datetime}/iteration-{iteration}/{substance}.obj"
        mtl_pattern: "{datetime}/iteration-{iteration}/{substance}.mtl"

      # Effect that blends a progression of PBR map samples
      # dependent on weathering degree over the original
      # materials of all entities with the specified materials.
      #
      # There can be multiple layer effects specified, in this
      # case, they will be applied in declaration order and
      # accumulate their effects, honoring alpha trasparency
      # in the PBR maps to model the amount of influence over
      # the original texture.
      #
      # Dimensions of output textures will be the same as the
      # texture size of the base material. If no base material
      # is defined on the entity, the dimensions of the largest
      # texture sample.
      - layer:
        # Apply the layer effect to entities of all materials
        # with a name of either "bronze" or "zinc". An empty
        # array or the material name "_" match all materials  # regardless of name.
        materials: ["bronze", "zinc"]
        # Synthesize the layer with the density of the "rust"
        # substance texture as the guide for blending the
        # texture samples before blending over the original
        # image.
        substance: "rust"
        # Margin around neighbourless edges in UV space to
        # avoid UV seam artifacts.
        island_bleed: 3
        # Modify the diffuse reflectivity or albedo of the
        # material by blending over samples.
        albedo:
          # Pattern for generated texture maps. Expressions
          # in braces get replaced to avoid naming conflicts.
          tex_pattern: "{datetime}/iteration-{iteration}/{id}-{entity}-{substance}-albedo.png"
          # List of texture samples specified with paths to
          # the sample images and the substance concentration
          # where their influence is maximal. For
          # concentrations in between ceniths, linear
          # interpolation between the neighbouring samples
          # is performed.
          # Alpha values in the textures indicate the amount
          # of influence over the base material.
          stops:
          - sample: "rust_stops/rust_nothing.png"
              cenith: 0.0
          - sample: "rust_stops/rust_alittle.png"
              cenith: 0.2
          - sample: "rust_stops/rust_alittlemore.png"
              cenith: 0.3
          - sample: "rust_stops/rust_notmuch.jpg"
              cenith: 0.4
          - sample: "rust_stops/rust_medium.jpg"
                cenith: 0.5
        # Also replace the metallicity of the input scene
        # with experimental map_Pm MTL key.
        metallicity:
          tex_pattern: "{datetime}/iteration-{iteration}/{id}-{entity}-{substance}-metallicity.png"
          stops:
          - sample: "white_512x512.png"
              cenith: 0.0
          - sample: "black_512x512.png"
              cenith: 0.7
        # Also replace the metallicity of the input scene
        # with experimental map_Pr MTL key.
        roughness:
          tex_pattern: "{datetime}/iteration-{iteration}/{id}-{entity}-{substance}-roughness.png"
          stops:
          - sample: "black_512x512.png"
            cenith: 0.0
          - sample: "white_512x512.png"
            cenith: 0.7
      # Serialize scenes with the effects of all layer effects
      # listed above the export declaration applied and new
      # materials generated for modified entities.
      - export:
        obj_pattern: "{datetime}/iteration-{iteration}/blent.obj"
        mtl_pattern: "{datetime}/iteration-{iteration}/blent.mtl"

## Ton Source Spec
Describes the properties of tons emitted by the source as well
as how the emission is performed.

Specifically, a ton source has an emission shape and tons
have initial materials as well as motion probabilities.

Here is an example of water drops being modelled by a rain
source:

    name: Rain
    description: Rain dropping from the sky
    # Path to OBJ describing the emission shape. Can be
    # identical with geometry in the scene, but should not
    # be placed inside objects in the simulation scene.
    mesh: sky.obj
    # Use normal direction on surface of sky.obj instead
    # of randomizing the direction over the upper hemisphere
    # at the immusion point as a value of true here would do.
    # False is the default.
    diffuse: false
    # Emit 100_000 particles per iteration.
    emission_count: 100000
    # Probability of the particles of moving further
    # in straight/parabolic/flow paths, respectively,
    # when interacting with a point on the surface.
    p_straight: 0.0
    p_parabolic: 0.3
    p_flow: 0.7
    # Initial concentration of substances in the tons
    initial:
      humidity: 1.0
      rust: 0.0
    # When interacting with the surface, percentage of
    # surface concentration, that will be picked up by
    # the ton. The value can also be negative to give
    # substance away to the surface instead of absorbing.
    absorb:
      humidity: 1.0
      rust: 0.2
    # Size of the particle in world space, indicating the range
    # in which it will interact with surfels
    interaction_radius: 0.1
    # When moving in a parabolic trajectory, maximum height
    # of such a parabola.
    parabola_height: 0.07
    # When flowing over a surface, the travelled distance.
    flow_distance: 0.17
    # When flowing, the direction to pull into. If unspecified,
    # will continue incident direction when flowing.
    flow_direction: [0.0, -1.0, 0.0]

## Surfel Spec
Surfel specs describe the properties of surfels that get
generated at the beginning of a simulation.

Other than original properties, the spec also describes
aging rules that get carried out at the end of each iteration,
before applying effects.

Such effects can transform substances from one type to the
other or deteriorate their concentrations over time.

Here is an example for iron with corrosion and evaporation
rules, referenced in the simulation spec above:

    # Meta
    name: Iron
    description: Surfel properties for metal-like substances with flow-like weathering artifacts
    # For each interaction, reduction in motion probabilities
    # of interacting gammatons.
    reflectance:
      delta_straight: 0.0
      delta_parabolic: 0.8
      delta_flow: 0.2
    # Initial concentration of substances on the surface.
    initial:
      humidity: 0.0
      rust: 0.0
    # When particles settle on this material, the percentage
    # of contained substance picked up by the surface from
    # the ton.
    deposit:
      # Rate of absorption from tons to this type of surfel
      humidity: 1.0
      rust: 0.5
    # Aging rules applied after each simulation iteration.
    rules:
      # Corrosion, remove humidity to make rust
      - from: humidity
        to: rust
        factor: 0.5
      # Evaportation reduces humidity without adding
      # it to something else.
      - from: humidity
        factor: -0.5



