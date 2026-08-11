struct Uniforms {
    yaw: f32,
    centre_x: f32,
    centre_y: f32,
    scale_x: f32,
    scale_y: f32,
    front_opacity: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var case_sampler: sampler;
@group(0) @binding(2) var front_texture: texture_2d<f32>;
@group(0) @binding(3) var rear_texture: texture_2d<f32>;
@group(0) @binding(4) var spine_texture: texture_2d<f32>;
@group(0) @binding(5) var from_texture: texture_2d<f32>;

// Keep the familiar ~10 mm jewel-case depth after trimming the unused clear
// bay from the visual shell. Half-depth 0.035 gives ~7% total depth.
const D: f32 = 0.035;
const CASE_WIDTH_MM: f32 = 135.0;
const CASE_HEIGHT_MM: f32 = 124.0;
const INSERT_EDGE_MM: f32 = 120.0;
// Meet the narrow tray at its moulded seam. The former 20 mm inset left
// 7.5 mm of empty clear lid between a 12.5 mm hinge and the booklet.
const INSERT_LEFT_MM: f32 = 13.0;
const INSERT_TOP_MM: f32 = 2.0;
const TRI: array<u32, 6> = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
const UV: array<vec2<f32>, 4> = array<vec2<f32>, 4>(
    vec2<f32>(0.0, 0.0), vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0), vec2<f32>(1.0, 0.0),
);
const POS: array<vec3<f32>, 24> = array<vec3<f32>, 24>(
    // front
    vec3<f32>(-0.5,  0.5,  D), vec3<f32>(-0.5, -0.5,  D),
    vec3<f32>( 0.5, -0.5,  D), vec3<f32>( 0.5,  0.5,  D),
    // rear, ordered as seen from behind so its insert is not mirrored
    vec3<f32>( 0.5,  0.5, -D), vec3<f32>( 0.5, -0.5, -D),
    vec3<f32>(-0.5, -0.5, -D), vec3<f32>(-0.5,  0.5, -D),
    // right spine
    vec3<f32>( 0.5,  0.5,  D), vec3<f32>( 0.5, -0.5,  D),
    vec3<f32>( 0.5, -0.5, -D), vec3<f32>( 0.5,  0.5, -D),
    // left spine
    vec3<f32>(-0.5,  0.5, -D), vec3<f32>(-0.5, -0.5, -D),
    vec3<f32>(-0.5, -0.5,  D), vec3<f32>(-0.5,  0.5,  D),
    // top tray
    vec3<f32>(-0.5,  0.5, -D), vec3<f32>(-0.5,  0.5,  D),
    vec3<f32>( 0.5,  0.5,  D), vec3<f32>( 0.5,  0.5, -D),
    // bottom tray
    vec3<f32>(-0.5, -0.5,  D), vec3<f32>(-0.5, -0.5, -D),
    vec3<f32>( 0.5, -0.5, -D), vec3<f32>( 0.5, -0.5,  D),
);
const NORMAL: array<vec3<f32>, 6> = array<vec3<f32>, 6>(
    vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 0.0, -1.0),
    vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(-1.0, 0.0, 0.0),
    vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, -1.0, 0.0),
);

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) @interpolate(flat) face: u32,
};

fn turn(value: vec3<f32>) -> vec3<f32> {
    let c = cos(uniforms.yaw);
    let s = sin(uniforms.yaw);
    return vec3<f32>(value.x * c + value.z * s, value.y, -value.x * s + value.z * c);
}

fn band(value: f32, centre: f32, half_width: f32, feather: f32) -> f32 {
    return 1.0 - smoothstep(half_width, half_width + feather, abs(value - centre));
}

fn case_edge(uv: vec2<f32>) -> f32 {
    return min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
}

fn rectangle_mask(uv: vec2<f32>, low: vec2<f32>, high: vec2<f32>, feather: f32) -> f32 {
    let inside_low = smoothstep(low, low + vec2<f32>(feather), uv);
    let inside_high = 1.0 - smoothstep(high - vec2<f32>(feather), high, uv);
    return inside_low.x * inside_low.y * inside_high.x * inside_high.y;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex: u32) -> VertexOut {
    var triangles = TRI;
    var positions = POS;
    var normals = NORMAL;
    var tex_coords = UV;
    let face = vertex / 6u;
    let corner = triangles[vertex % 6u];
    let world = turn(positions[face * 4u + corner]);
    let normal = turn(normals[face]);
    let view_z = 2.55 - world.z;
    let lens = 4.0;
    var out: VertexOut;
    out.position = vec4<f32>(
        uniforms.centre_x * view_z + world.x * lens * uniforms.scale_x,
        uniforms.centre_y * view_z + world.y * lens * uniforms.scale_y,
        0.5 * view_z,
        view_z,
    );
    out.uv = tex_coords[corner];
    out.world = world;
    out.normal = normal;
    out.face = face;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let camera = vec3<f32>(0.0, 0.0, 2.55);
    if (dot(in.normal, normalize(camera - in.world)) <= 0.0) {
        discard;
    }

    var color: vec4<f32>;
    if (in.face == 0u) {
        // A front booklet is 120 × 120 mm inside the fitted 135 × 124 mm case. Keeping
        // this mapping physical prevents the cover being stretched merely to
        // fill the case's wider silhouette.
        let physical_uv = in.uv * vec2<f32>(CASE_WIDTH_MM, CASE_HEIGHT_MM);
        let cover_low = vec2<f32>(INSERT_LEFT_MM, INSERT_TOP_MM);
        let cover_high = cover_low + vec2<f32>(INSERT_EDGE_MM);
        let cover_uv = clamp((physical_uv - cover_low) / vec2<f32>(INSERT_EDGE_MM),
            vec2<f32>(0.0), vec2<f32>(1.0));
        let cover_mask = rectangle_mask(physical_uv, cover_low, cover_high, 0.5);
        let view = normalize(camera - in.world);
        let facing = clamp(dot(normalize(in.normal), view), 0.0, 1.0);
        // The booklet sits beneath the lid. Its tiny view-dependent displacement
        // is the depth cue that distinguishes clear plastic from a gloss painted
        // directly onto the artwork.
        let parallax = vec2<f32>(-view.x, view.y) * (1.0 - facing) * 0.018;
        let refracted_uv = clamp(cover_uv + parallax, vec2<f32>(0.0), vec2<f32>(1.0));
        let previous = textureSample(from_texture, case_sampler, refracted_uv);
        let front = textureSample(front_texture, case_sampler, refracted_uv);
        let cover = mix(previous, front, uniforms.front_opacity);
        color = vec4<f32>(
            mix(vec3<f32>(0.12, 0.135, 0.145), cover.rgb, cover_mask),
            mix(0.18, cover.a, cover_mask),
        );

        // The black rear tray is visible through the clear lid along the hinge.
        // The opaque tray occupies only the inner part of the wider clear
        // hinge bay; making the whole 20 mm booklet margin black overstates it.
        let hinge = 1.0 - smoothstep(0.068, 13.0 / CASE_WIDTH_MM, in.uv.x);
        let hinge_seam = band(in.uv.x, 13.0 / CASE_WIDTH_MM, 0.0015, 0.003);
        // Fine ribs run with the hinge. They are moulded into the tray rather
        // than being two horizontal clips painted across it.
        let hinge_ribs = band(in.uv.x, 0.0140, 0.0008, 0.0014)
            + band(in.uv.x, 0.0233, 0.0008, 0.0014)
            + band(in.uv.x, 0.0327, 0.0008, 0.0014)
            + band(in.uv.x, 0.0420, 0.0008, 0.0014)
            + band(in.uv.x, 0.0513, 0.0008, 0.0014)
            + band(in.uv.x, 0.0607, 0.0008, 0.0014)
            + band(in.uv.x, 0.0700, 0.0008, 0.0014);
        let tray = vec3<f32>(0.018, 0.020, 0.023);
        color = vec4<f32>(mix(color.rgb, tray, hinge * 0.92), color.a);
        color = vec4<f32>(color.rgb, max(color.a, hinge * 0.96));
        color = vec4<f32>(
            mix(color.rgb, vec3<f32>(0.14, 0.15, 0.16),
                hinge_seam * 0.48 + hinge_ribs * hinge * 0.18),
            color.a,
        );

        // Three layers of clear plastic: the case rim, a raised bevel around the
        // booklet window, and two separated reflections on the lid's surface.
        let edge = case_edge(in.uv);
        let outer_rim = 1.0 - smoothstep(0.003, 0.012, edge);
        let cover_edge = min(min(cover_uv.x, 1.0 - cover_uv.x),
            min(cover_uv.y, 1.0 - cover_uv.y));
        let raised_bevel = 1.0 - smoothstep(0.002, 0.012, cover_edge);
        let inner_lip = band(cover_edge, 0.016, 0.0015, 0.004);
        let lit_bevel = band(cover_uv.x, 0.005, 0.004, 0.010)
            + band(cover_uv.y, 0.005, 0.004, 0.010);
        let shaded_bevel = band(cover_uv.x, 0.995, 0.004, 0.010)
            + band(cover_uv.y, 0.995, 0.004, 0.010);
        // Until the lid is separate geometry, keep the environment streaks
        // at its perimeter where a thin clear layer can plausibly catch them.
        let edge_reflection = 1.0 - smoothstep(0.018, 0.075, cover_edge);
        let diagonal = band(in.uv.x + in.uv.y * 0.20, 0.24, 0.025, 0.07);
        let reflected_diagonal = band(
            in.uv.x + in.uv.y * 0.20 + view.x * 0.035,
            0.27,
            0.010,
            0.035,
        );
        let fine_glint = band(
            in.uv.x - in.uv.y * 0.11 - view.x * 0.025,
            0.76,
            0.006,
            0.018,
        );
        let grazing = pow(1.0 - abs(dot(normalize(in.normal), view)), 3.0);
        // The opaque hinge is tray plastic, not part of the clear lid's pane.
        // Keep its moulded seam/ribs above and confine glass reflections to
        // the booklet window so white glints do not wash over the black bay.
        color = vec4<f32>(
            color.rgb * (1.0 - shaded_bevel * cover_mask * 0.10),
            color.a,
        );
        let glass = (outer_rim * 0.30 + raised_bevel * 0.12
            + inner_lip * 0.18 + lit_bevel * 0.16
            + (diagonal * 0.045 + reflected_diagonal * 0.075
                + fine_glint * 0.065) * edge_reflection
            + grazing * 0.08) * cover_mask;
        color = vec4<f32>(
            mix(color.rgb, vec3<f32>(0.94, 0.975, 1.0), min(glass, 0.58)),
            color.a,
        );
    } else if (in.face == 1u) {
        color = textureSample(rear_texture, case_sampler, in.uv);

        let edge = case_edge(in.uv);
        let outer_rim = 1.0 - smoothstep(0.006, 0.024, edge);
        let inner_lip = band(edge, 0.031, 0.0015, 0.0035);
        let reflection = band(in.uv.x + in.uv.y * 0.16, 0.88, 0.035, 0.08);
        let view = normalize(camera - in.world);
        let grazing = pow(1.0 - abs(dot(normalize(in.normal), view)), 3.0);
        color = vec4<f32>(
            mix(color.rgb, vec3<f32>(0.90, 0.95, 0.98),
                min(outer_rim * 0.24 + inner_lip * 0.12
                    + reflection * 0.035 + grazing * 0.09, 0.42)),
            color.a,
        );
    } else if (in.face == 2u || in.face == 3u) {
        // Both outside edges carry the rear inlay's printed spine beneath the
        // clear shell. The black tray is the front hinge bay, not a substitute
        // for the artist/title strip on the case's outer edge.
        var spine_uv = in.uv;
        if (in.face == 3u) {
            spine_uv = vec2<f32>(1.0 - in.uv.x, in.uv.y);
        }
        color = textureSample(spine_texture, case_sampler, spine_uv);
        let edge_glint = band(in.uv.x, 0.10, 0.035, 0.08)
            + band(in.uv.x, 0.90, 0.035, 0.08);
        color = vec4<f32>(
            mix(color.rgb * (0.80 + 0.20 * abs(in.normal.z)), vec3<f32>(0.86, 0.91, 0.94),
                min(edge_glint * 0.12, 0.20)),
            color.a,
        );
    } else {
        // Top and bottom expose the tray under a narrow clear shell highlight.
        let shell = band(in.uv.x, 0.03, 0.025, 0.05)
            + band(in.uv.x, 0.97, 0.025, 0.05);
        color = vec4<f32>(
            vec3<f32>(0.024, 0.026, 0.030)
                + vec3<f32>(0.11, 0.13, 0.14) * min(shell * 0.45, 1.0),
            1.0,
        );
    }
    return color;
}
