# Documentation

| I want to... | Read |
|--------------|------|
| use the API | [user/api.md](user/api.md) |
| change a setting | [user/configuration.md](user/configuration.md) |
| run it in Docker or deploy it | [user/deploy.md](user/deploy.md) |
| understand how it works | [architecture.md](architecture.md) |
| follow how it was built, step by step | [development/](development/README.md) |
| see what is planned and why | [../PLAN.md](../PLAN.md) and [plan/](plan/phase-1.md) |

## Layout

```
docs/
├── README.md              this page
├── architecture.md        what the service is, how the parts fit together, code map, limits
├── user/
│   ├── api.md             running it, every endpoint with examples, errors
│   ├── configuration.md   environment variables
│   └── deploy.md          Docker image and deployment on the home server
├── development/
│   ├── README.md          workflow, tests, chapter index and template
│   └── NN-*.md            one chapter per development step, in order
└── plan/
    ├── phase-1.md         detailed plan of Phase 1 (done)
    ├── phase-2.md         Phase 2: web app and first deployment (in development)
    └── phase-3.md         Phase 3: rules, trip tracking, Web Push (decisions confirmed)
```

## Keeping it current

- Every development step adds a chapter in `development/` in the same PR as the code (template in [development/README.md](development/README.md#writing-a-chapter)).
- A change visible to users also updates `user/` and, when the design changes, `architecture.md`.
- Each phase has its plan in `plan/phase-N.md`, written and approved before the phase starts and updated if the plan changes.
- The `docs-sync` skill, a commit hook and a CI check keep this true: see [development/README.md](development/README.md#keeping-the-docs-in-sync).
