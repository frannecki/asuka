---
name: generate-diagrams
description: Generate vivid, comic-like Technical Architecture Diagrams from codebase analysis. Use when the user wants to visualize system architecture, module relationships, or data flows with more energy, stronger color, and poster-like clarity.
---

# Generate Architecture Diagrams

This skill provides a Python script to generate high-quality Technical Architecture Diagrams using AI image generation. It favors a vivid comic-tech poster aesthetic: bold composition, stronger color separation, dynamic flow arrows, expressive callouts, richer iconography, and still-readable technical detail.

## Available Resources

- `scripts/generate_diagrams.py`: A Python script to generate architecture diagrams.

## ⚠️ IMPORTANT: Unified Request Format

**This script ONLY accepts the `--requests` parameter with a JSON array format.**

All diagram generation requests MUST be passed as a JSON array, **even for generating a single diagram**. This ensures consistent parameter handling and enables parallel execution for multiple diagrams.

## Design Principles

### 1. Comprehensiveness & Scope

- **No Component Left Behind**: Scan the entire codebase or project scope. Ensure ALL major modules, sub-systems, background workers (Cron/Async), and auxiliary services (Logging, Auth, Config) are represented.
- **Traffic Sources**: Clearly define where the data comes from. Don't just say "User". Specify the client types (e.g., "Web Console", "Mobile SDK", "3rd Party Webhook", "Internal Scheduler").

### 2. Detail Density (The "Zoom-In" Effect)

- **No Empty Boxes**: A module box containing only a title is unacceptable.
- **Internal Anatomy**: Inside every module's container, visualize its internals using:
  - **Text Lists**: Listing key classes/interfaces (e.g., `TokenManager`, `RateLimiter`).
  - **Mini-Flows**: Showing the internal pipeline (e.g., `Parser -> Compiler -> Executor`).
  - **Nested Components**: Sub-modules inside the parent module.

### 3. Logical Structuring (Adaptable Layers)

- **Context-Aware Layering**: Adapt the layers to the specific project pattern (MVC, DDD, Microservices, or Hexagonal). A typical flow includes:
  1. **Entry/Interface**: Gateways, API handlers, Event Listeners.
  2. **Application/Orchestration**: Workflow managers, business use-cases.
  3. **Core Domain**: The business logic, algorithms, state machines.
  4. **Infrastructure/Adapters**: Clients for DBs, external APIs, common utilities.
- **Flow Direction**: Use arrows to clearly show data flow. Arrows should tell a story (e.g., `Request -> Processing -> Storage`).

### 4. Visual Aesthetics & Clarity

- **Color Coding**: Use high-contrast grouped colors to separate layers or functional areas. Favor richer, more vivid tones over muted enterprise palettes.
  - **Rule**: Do not write the color name in the text label. Let the visual color speak for itself.
- **Style**: Prefer a "Comic-Tech Poster", "Graphic-Novel Architecture Board", or "Illustrated Technical Schematic" feel.
- **Energy**: Use dynamic arrows, punchy captions, panel-like grouping, subtle depth, and stronger silhouettes so the diagram feels lively instead of static.
- **No Poster Title**: Do not place a large top-level title, banner title, or decorative heading inside the generated image. The diagram itself should start with the architecture content.
- **Icon Density**: Use more icons and pictograms to represent services, databases, clouds, queues, APIs, files, browsers, bots, and tools. Do not rely on boxes, arrows, and text alone.
- **Textures**: Light halftone, dot-grid, annotation, or poster textures are acceptable if they do not reduce legibility.
- **Legibility**: Text must be crisp and professional. Lines should be straight and organized (orthogonal routing preferred).

### 5. Naming Conventions

- **Professional Terminology**: Use exact technical names from the code/tech stack (e.g., use "Redis Sentinel" instead of "Cache", "gRPC Handler" instead of "API").
- **Remove Redundancy**: Avoid generic suffixes like "Box", "Container", or "Block" in titles.

## Usage

To generate architecture diagrams, execute the `generate_diagrams.py` script using `python3`.

### Requirements

- Python 3.x must be installed.
- `OPENROUTER_API_KEY` environment variable must be set (or passed via `--api-key`).

### Command Format

```bash
python3 <path-to-script>/generate_diagrams.py --requests '<JSON_ARRAY>' [--parallel N] [--api-key KEY]
```

### Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| `--requests` | **Yes** | JSON array of request objects (required even for single diagram) |
| `--parallel` | No | Max parallel workers for concurrent generation (default: 3) |
| `--api-key` | No | API Key override (defaults to `OPENROUTER_API_KEY` env var) |

### Request Object Schema

Each object in the `--requests` JSON array should follow this structure:

```json
{
  "project": "Project Name",
  "description": "Brief description of the system",
  "components": [...],
  "outputFile": "output.png",
  "style": "blueprint|swiss|schematic",
  "layers": "mvc|ddd|microservices|hexagonal"
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `project` | **Yes** | Project or system name |
| `description` | No | Brief description of the system's purpose |
| `components` | No | JSON array of component definitions with internals |
| `outputFile` | No | Output filename (defaults to `{project}_arch.png`) |
| `style` | No | Visual style (default: `blueprint`, but render it as a vivid comic-tech blueprint rather than a muted corporate blueprint) |
| `layers` | No | Architecture pattern |

### Component Definition Schema

Each component in the `components` array should follow this structure:

```json
{
  "name": "Service Name",
  "type": "entry|logic|data|infra",
  "internals": ["SubComponent1", "SubComponent2"],
  "connections": ["Target Service 1", "Target Service 2"]
}
```

**Component Types:**

| Type | Description | Color Theme |
|------|-------------|-------------|
| `entry` | API Gateways, Event Listeners, User Interfaces | Electric Blue |
| `logic` | Business Logic, Workflow Managers, Domain Services | Mint Green |
| `data` | Databases, Caches, Message Queues | Vivid Lavender |
| `infra` | External APIs, Cloud Services, Utilities | Warm Slate |

## Examples

### Single Diagram Generation

Even for a single diagram, you MUST use `--requests` with a JSON array:

```bash
python3 <path-to-script>/generate_diagrams.py --requests '[{
  "project": "OnCall Management System",
  "description": "A system for managing on-call schedules, incident alerts, and escalation policies",
  "outputFile": "architecture.png"
}]'
```

### Single Diagram with Components

```bash
python3 <path-to-script>/generate_diagrams.py --requests '[{
  "project": "E-Commerce Platform",
  "components": [
    {"name": "API Gateway", "type": "entry", "internals": ["AuthMiddleware", "RateLimiter", "RequestRouter"]},
    {"name": "Order Service", "type": "logic", "internals": ["OrderProcessor", "PaymentHandler", "InventoryChecker"]},
    {"name": "PostgreSQL", "type": "data", "internals": ["Orders Table", "Users Table", "Products Table"]}
  ],
  "outputFile": "ecommerce_arch.png"
}]'
```

### Multiple Diagrams (Parallel Generation)

Generate multiple diagrams concurrently:

```bash
python3 <path-to-script>/generate_diagrams.py --requests '[
  {"project": "Auth Service", "description": "JWT-based authentication", "outputFile": "auth_arch.png"},
  {"project": "Payment Gateway", "description": "Payment processing with Stripe", "outputFile": "payment_arch.png"},
  {"project": "Notification Service", "description": "Multi-channel notification delivery", "outputFile": "notification_arch.png"}
]' --parallel 5
```

### Full Component Specification Example

```bash
python3 <path-to-script>/generate_diagrams.py --requests '[{
  "project": "OnCall Platform",
  "description": "Intelligent on-call management and incident response system",
  "style": "blueprint",
  "layers": "microservices",
  "components": [
    {"name": "Web Console", "type": "entry", "internals": ["React SPA", "WebSocket Client"]},
    {"name": "API Gateway", "type": "entry", "internals": ["Hertz Handler", "JWT Validator", "Rate Limiter"]},
    {"name": "Schedule Service", "type": "logic", "internals": ["ScheduleManager", "RotationEngine", "ConflictResolver"]},
    {"name": "Alert Service", "type": "logic", "internals": ["AlertDispatcher", "EscalationPolicy", "NotificationRouter"]},
    {"name": "MySQL Cluster", "type": "data", "internals": ["oncall_schedule", "oncall_user", "oncall_alert"]},
    {"name": "Redis Sentinel", "type": "data", "internals": ["Session Cache", "Rate Limit Counter"]},
    {"name": "Kafka", "type": "data", "internals": ["alert-events", "schedule-changes"]},
    {"name": "Lark Bot", "type": "infra", "internals": ["Message API", "Card Builder"]},
    {"name": "SMS Gateway", "type": "infra", "internals": ["Twilio Client", "Template Engine"]}
  ],
  "outputFile": "oncall_architecture.png"
}]'
```

## Prompt Template Direction

The script internally uses this optimized prompt template for generating diagrams:

```
Task: Generate a comprehensive Technical Architecture Diagram for [Project Name].

Description: [Project Description]

Style: Vivid comic-tech architecture poster. Bold layout, crisp lettering, richer colors,
panel-like grouping, dynamic data-flow arrows, icon-rich composition, clean background,
and high resolution.

Layout Strategy:
1. Top Layer (Entry): Show all traffic sources (Users, Systems, Triggers).
2. Middle Layers (Logic): Group modules by function.
   - Requirement: For every module box, render text/icons inside it showing its 
     sub-components or processing steps.
3. Bottom Layer (Data/Infra): Show databases, queues, and external cloud services.

Visual Cues:
- Use arrows to show the flow of data.
- Color-code layers logically with stronger saturation and clearer contrast.
- Make the composition feel energetic and poster-like, not dull or overly corporate.
- Do not include a large title rendered inside the image.
- Use icons generously for systems and concepts instead of only boxes and labels.
- Ensure all text labels are legible and professional.
- Use orthogonal routing for connection lines.

Components:
[Component Details]
```
