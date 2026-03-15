import os
import sys
import json
import argparse
import urllib.request
import urllib.error
import base64
import time
import concurrent.futures

DEFAULT_BASE_URL = 'https://openrouter.ai/api/v1/chat/completions'
DEFAULT_MODEL = 'google/gemini-3.1-flash-image-preview'

STYLE_CONFIGS = {
    'blueprint': {
        'name': 'Vivid Comic-Tech Blueprint',
        'description': 'High-energy technical blueprint with stronger color, expressive callouts, and comic-poster clarity'
    },
    'swiss': {
        'name': 'Editorial Swiss Comic Infographic',
        'description': 'Grid-based Swiss composition with brighter contrast, sharp typography, and graphic-novel energy'
    },
    'schematic': {
        'name': 'Illustrated Technical Schematic',
        'description': 'Systematic technical schematic with bold outlines, layered callouts, and vivid diagram storytelling'
    }
}

LAYER_CONFIGS = {
    'mvc': {
        'name': 'MVC Architecture',
        'layers': ['View/Controller', 'Model/Service', 'Data Access']
    },
    'ddd': {
        'name': 'Domain-Driven Design',
        'layers': ['Interface', 'Application', 'Domain', 'Infrastructure']
    },
    'microservices': {
        'name': 'Microservices Architecture',
        'layers': ['API Gateway', 'Services', 'Data Stores', 'External Systems']
    },
    'hexagonal': {
        'name': 'Hexagonal Architecture',
        'layers': ['Adapters (Driving)', 'Application Core', 'Adapters (Driven)']
    }
}

COMPONENT_COLORS = {
    'entry': 'Electric Blue (#5DA9FF)',
    'logic': 'Mint Green (#7EDC9A)',
    'data': 'Vivid Lavender (#C59BFF)',
    'infra': 'Warm Slate (#B8C2CC)'
}


def build_prompt(project, description=None, components=None, style='blueprint', layers=None):
    style_config = STYLE_CONFIGS.get(style, STYLE_CONFIGS['blueprint'])
    layer_config = LAYER_CONFIGS.get(layers) if layers else None

    prompt_parts = [
        f"Task: Generate a comprehensive Technical Architecture Diagram for {project}.",
        ""
    ]

    if description:
        prompt_parts.extend([
            f"Description: {description}",
            ""
        ])

    prompt_parts.extend([
        f"Style: {style_config['name']}. {style_config['description']}.",
        "Use a vivid comic-tech palette with strong contrast, bold grouping, dynamic composition, and icon-rich storytelling.",
        "Favor a clean bright background with graphic-novel energy, subtle poster texture, and high resolution.",
        "Keep the result professional, readable, and suitable for technical reviews.",
        ""
    ])

    if layer_config:
        prompt_parts.extend([
            f"Architecture Pattern: {layer_config['name']}",
            f"Layers: {' -> '.join(layer_config['layers'])}",
            ""
        ])

    prompt_parts.extend([
        "Layout Strategy:",
        "1. Top Layer (Entry): Show all traffic sources (Users, Systems, Triggers, Web Console, Mobile SDK, Webhooks, Internal Scheduler).",
        "2. Middle Layers (Logic): Group modules by function.",
        "   - CRITICAL: For every module box, render text/icons inside it showing its sub-components or processing steps.",
        "   - No empty boxes allowed - each module must show its internal anatomy.",
        "3. Bottom Layer (Data/Infra): Show databases, queues, message brokers, and external cloud services.",
        ""
    ])

    prompt_parts.extend([
        "Visual Requirements:",
        "- Use arrows to show the flow of data. Arrows should tell a story (Request -> Processing -> Storage).",
        "- Make the composition feel lively, vivid, and comic-like rather than flat or corporate.",
        "- Do NOT render a large title, top banner heading, or decorative project title inside the image.",
        "- Use panel-like grouping, punchy callouts, bold outlines, expressive routing, and more icons where it improves clarity.",
        "- Prefer recognizable icons/pictograms for browsers, users, APIs, services, databases, queues, clouds, files, bots, tools, and storage systems.",
        "- Avoid diagrams that are only boxes, edges, and text; icon support should be visibly present across the composition.",
        "- Color-code layers logically with richer saturation and stronger separation:",
        f"  - Entry/Interface components: {COMPONENT_COLORS['entry']}",
        f"  - Logic/Domain components: {COMPONENT_COLORS['logic']}",
        f"  - Data/Storage components: {COMPONENT_COLORS['data']}",
        f"  - Infrastructure/External: {COMPONENT_COLORS['infra']}",
        "- Do NOT write color names in labels. Let visual colors speak for themselves.",
        "- Ensure all text labels are legible and professional.",
        "- Preserve architecture rigor: no childish mascots, no decorative clutter, no illegible stylization.",
        "- Use orthogonal routing for connection lines (straight lines with 90-degree turns).",
        "- Use exact technical names (e.g., 'Redis Sentinel' not 'Cache', 'gRPC Handler' not 'API').",
        "- Avoid generic suffixes like 'Box', 'Container', or 'Block' in titles.",
        ""
    ])

    if components:
        prompt_parts.extend([
            "Components to Include:",
            ""
        ])

        for comp_type in ['entry', 'logic', 'data', 'infra']:
            type_components = [c for c in components if c.get('type') == comp_type]
            if type_components:
                type_label = {
                    'entry': 'Entry/Interface Layer',
                    'logic': 'Logic/Domain Layer',
                    'data': 'Data/Storage Layer',
                    'infra': 'Infrastructure/External'
                }.get(comp_type, comp_type)

                prompt_parts.append(f"[{type_label}]")
                for comp in type_components:
                    name = comp.get('name', 'Unknown')
                    internals = comp.get('internals', [])
                    connections = comp.get('connections', [])

                    if internals:
                        internals_str = ', '.join(internals)
                        prompt_parts.append(f"- {name}: Contains [{internals_str}]")
                    else:
                        prompt_parts.append(f"- {name}")

                    if connections:
                        prompt_parts.append(f"  Connects to: {', '.join(connections)}")

                prompt_parts.append("")

    prompt_parts.extend([
        "Final Check:",
        "- Verify ALL components are represented.",
        "- Verify NO module box is empty - each must show internal details.",
        "- Verify data flow arrows are clear and logical.",
        "- Verify color coding is consistent across layers."
    ])

    return '\n'.join(prompt_parts)


def generate_diagram(request, api_key=None, base_url=None, model=None):
    project = request.get('project')
    description = request.get('description')
    components = request.get('components')
    output_file = request.get('outputFile', f"{project.lower().replace(' ', '_')}_arch.png")
    style = request.get('style', 'blueprint')
    layers = request.get('layers')
    aspect_ratio = request.get('aspectRatio', '16:9')

    api_key = api_key or os.environ.get('OPENROUTER_API_KEY')
    base_url = base_url or os.environ.get('OPENROUTER_BASE_URL', DEFAULT_BASE_URL)
    model = model or os.environ.get('NANOBANANA_MODEL', DEFAULT_MODEL)

    if not api_key:
        return False, "Error: API key not found. Set OPENROUTER_API_KEY env var or pass --api-key."

    prompt = build_prompt(project, description, components, style, layers)
    if aspect_ratio and aspect_ratio != "1:1":
        prompt += f"\n\n(Aspect Ratio: {aspect_ratio})"

    payload = {
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": prompt
            }
        ],
        "modalities": ["image", "text"]
    }

    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {api_key}",
        "HTTP-Referer": "gemini-cli - Overview",
        "X-Title": "Gemini CLI"
    }

    try:
        req = urllib.request.Request(base_url, data=json.dumps(payload).encode('utf-8'), headers=headers, method='POST')

        with urllib.request.urlopen(req, timeout=300) as response:
            response_body = response.read().decode('utf-8')
            data = json.loads(response_body)

            if 'choices' in data and len(data['choices']) > 0:
                message = data['choices'][0]['message']
                content = message.get('content', '') or ''

                base64_data = None

                if 'images' in message and len(message['images']) > 0:
                    img_obj = message['images'][0]
                    if 'image_url' in img_obj and 'url' in img_obj['image_url']:
                        image_url = img_obj['image_url']['url']
                        if image_url.startswith("data:"):
                            try:
                                base64_data = image_url.split(",")[1]
                            except Exception:
                                pass

                if not base64_data:
                    import re
                    match = re.search(r'data:image/(\w+);base64,([^")\s]+)', content)
                    if match:
                        base64_data = match.group(2)

                if base64_data:
                    output_dir = os.path.dirname(os.path.abspath(output_file))
                    if output_dir:
                        os.makedirs(output_dir, exist_ok=True)
                    with open(output_file, 'wb') as f:
                        f.write(base64.b64decode(base64_data))

                    return True, f"Success: Architecture diagram saved to {output_file}"
                else:
                    return False, f"Failed: No image data found. Keys in message: {list(message.keys())}. Content: {content[:100]}..."
            else:
                return False, f"Failed: Invalid response structure. Response: {response_body[:200]}"

    except urllib.error.HTTPError as e:
        error_body = e.read().decode('utf-8')
        return False, f"HTTP Error {e.code}: {e.reason} - Response: {error_body}"
    except Exception as e:
        return False, f"Error generating diagram for '{project}': {str(e)}"


def main():
    parser = argparse.ArgumentParser(
        description='Generate Technical Architecture Diagrams (Parallel Execution)',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog='''
IMPORTANT: This script ONLY accepts --requests parameter with JSON array format.
All diagram generation requests must be passed as a JSON array, even for a single diagram.

Examples:
  # Single diagram (must use --requests with JSON array)
  python generate_diagrams.py --requests '[{
    "project": "My Service",
    "description": "A REST API service",
    "outputFile": "arch.png"
  }]'

  # Single diagram with components
  python generate_diagrams.py --requests '[{
    "project": "My Service",
    "components": [{"name": "API", "type": "entry", "internals": ["Router", "Auth"]}],
    "outputFile": "arch.png"
  }]'

  # Multiple diagrams (parallel generation)
  python generate_diagrams.py --requests '[
    {"project": "Service A", "description": "Auth service", "outputFile": "a.png"},
    {"project": "Service B", "description": "Payment service", "outputFile": "b.png"}
  ]' --parallel 5

Request Object Schema:
  {
    "project": "Project Name",           # Required
    "description": "Brief description",  # Optional
    "components": [...],                 # Optional, JSON array of component definitions
    "outputFile": "output.png",          # Optional, defaults to {project}_arch.png
    "style": "blueprint|swiss|schematic", # Optional, defaults to "blueprint"
    "layers": "mvc|ddd|microservices|hexagonal" # Optional
  }

Component Definition Schema:
  {
    "name": "Component Name",
    "type": "entry|logic|data|infra",
    "internals": ["SubComponent1", "SubComponent2"],
    "connections": ["Target1", "Target2"]
  }
        '''
    )

    parser.add_argument('--requests', type=str, required=True,
                        help='JSON array of request objects (required, even for single diagram)')
    parser.add_argument('--api-key', type=str, 
                        help='API Key (optional, defaults to OPENROUTER_API_KEY env var)')
    parser.add_argument('--parallel', type=int, default=3, 
                        help='Max parallel workers for concurrent generation (default: 3)')

    args = parser.parse_args()

    try:
        requests_list = json.loads(args.requests)
    except json.JSONDecodeError as e:
        print(f"Error: Invalid JSON in --requests: {e}")
        sys.exit(1)

    if not isinstance(requests_list, list):
        print("Error: --requests must be a JSON array")
        sys.exit(1)

    if len(requests_list) == 0:
        print("Error: --requests array is empty")
        sys.exit(1)

    for i, req in enumerate(requests_list):
        if not req.get('project'):
            print(f"Error: Request at index {i} is missing required 'project' field")
            sys.exit(1)

    print(f"Starting parallel generation for {len(requests_list)} diagram(s) with {args.parallel} workers...")

    start_time = time.time()
    success_count = 0

    with concurrent.futures.ThreadPoolExecutor(max_workers=args.parallel) as executor:
        future_to_req = {
            executor.submit(generate_diagram, req, args.api_key): req 
            for req in requests_list
        }

        for future in concurrent.futures.as_completed(future_to_req):
            req = future_to_req[future]
            success, message = future.result()
            print(f"[{req.get('project')}] {message}")
            if success:
                success_count += 1

    duration = time.time() - start_time
    print(f"\nSummary: Generated {success_count}/{len(requests_list)} diagram(s) in {duration:.2f}s.")

    if success_count < len(requests_list):
        sys.exit(1)


if __name__ == "__main__":
    main()
