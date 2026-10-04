#version 330 core

in vec2 texCoord0;
in vec3 viewNormal;
in vec3 viewPos;

uniform sampler2D Sampler0;
uniform vec3 OutlineColor;
uniform float AlphaCutoff;
uniform float GlobalAlpha;

/* TOON_OUTPUT_DECLARATIONS */
/* TOON_OUTPUT_WRITER */

void main() {
    vec4 texColor = texture(Sampler0, texCoord0);
    if (texColor.a < AlphaCutoff) {
        discard;
    }

    vec3 normal = normalize(viewNormal);
    vec3 viewDir = normalize(-viewPos);
    float facing = dot(normal, viewDir);

    float silhouette = 1.0 - clamp(facing, 0.0, 1.0);
    float edge = smoothstep(0.08, 0.45, silhouette);

    if (edge < 0.01) {
        discard;
    }

    float texLum = dot(texColor.rgb, vec3(0.299, 0.587, 0.114));
    vec3 finalOutline = OutlineColor * mix(1.0, texLum, 0.4);

    float finalAlpha = texColor.a * edge * clamp(GlobalAlpha, 0.0, 1.0);
    if (finalAlpha < 0.001) {
        discard;
    }

    writeToonOutputs(finalOutline, finalOutline, normal, finalAlpha);
}
