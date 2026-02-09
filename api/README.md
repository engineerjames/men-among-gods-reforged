# Authentication Service

This project is an authentication service implemented in Rust. It provides functionalities for user authentication, including user registration and token generation.

# KeyDB setup
The authentication service relies on KeyDB (a high-performance fork of Redis) for storing user data and session information. To set up KeyDB, follow these steps:
1. Install KeyDB: You can download and install KeyDB from the official website: https://keydb.dev/.
2. Start KeyDB: Once installed, start the KeyDB server using the command: `keydb-server --port 5556`.

Eventually we'll have a customized configuration file that is known to work well with the server, but for now the default configuration should work fine.

# Server token generation
The server generates authentication tokens using the `jsonwebtoken` crate. When a user successfully logs in, the server creates a JWT (JSON Web Token) that contains the user's information and an expiration time. The token is signed using a secret key, which is stored securely on the server. The generated token is then sent back to the client, which can use it for subsequent authenticated requests to the server.

The server must have an environment variable defined for the secret key used in token generation. You can set this environment variable in your terminal before running the server:

```bash
export API_JWT_SECRET="your_secret_key_here"
```

You can generate the secret (32+bytes) using either command:

```bash
openssl rand -hex 32
```

or

```bash
python -c "import secrets; print(secrets.token_hex(32))"
```   

# KeyDB data model (key/value layout)

This service stores accounts and characters in KeyDB using a small set of predictable key patterns.

## Key patterns

```mermaid
flowchart TB
    %% Account ID allocation
    A_NEXT["account:next_id\nString (integer)"]
    A_HASH["account:{account_id}\nHASH\n- id\n- email\n- username\n- password"]

    %% Account lookup indexes
    A_EMAIL["account:email:{email}\nString -> account_id"]
    A_USER["account:username:{username}\nString -> account_id"]

    %% Characters owned by an account
    A_CHARS["account:{account_id}:characters\nSET<character_id>"]

    %% Character ID allocation + character storage
    C_NEXT["character:next_id\nString (integer)"]
    C_HASH["character:{character_id}\nHASH\n- account_id\n- name\n- description\n- sex\n- race"]

    A_NEXT -->|INCR| A_HASH
    A_EMAIL -->|GET| A_HASH
    A_USER -->|GET| A_HASH

    A_HASH -->|owns| A_CHARS
    A_CHARS -->|ids| C_HASH
    C_NEXT -->|INCR| C_HASH

    C_HASH -. "account_id field" .-> A_HASH
```

## Relationships (conceptual)

```mermaid
erDiagram
    ACCOUNT ||--o{ CHARACTER : owns

    ACCOUNT {
        u64 id
        string email
        string username
        string password
    }

    CHARACTER {
        u64 id
        u64 account_id
        string name
        string description
        u32 sex
        u32 race
    }
```

Notes:
- The `ACCOUNT -> CHARACTER` relationship is materialized primarily via `account:{account_id}:characters` (a set of character IDs), and redundantly via the `account_id` field stored on `character:{character_id}`.
- Username/email uniqueness is enforced via the index keys: `account:username:{username}` and `account:email:{email}`.

# Client auth + JWT usage flow

This is the intended client sequence to authenticate via `/login`, receive a JWT, and then call the validated endpoints for character list/create/update/delete.

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant API as Auth Service

    C->>API: POST /login { username, password }
    alt Login fails
        API-->>C: 401 Unauthorized
    else Login succeeds
        API-->>C: 200 { token: JWT(sub=username, exp=now+3600) }
    end

    Note over C: Store token locally
    Note over C,API: Send on subsequent requests\nAuthorization: Bearer <JWT>

    C->>API: GET /characters (Authorization)
    API->>API: verify_token(JWT, API_JWT_SECRET)
    API-->>C: 200 { characters: [...] }

    C->>API: POST /characters (Authorization, CreateCharacterRequest)
    API->>API: verify_token(...)
    API-->>C: 200 CharacterSummary

    C->>API: PUT /characters/{id} (Authorization, UpdateCharacterRequest{name?, description?})
    API->>API: verify_token(...)
    alt Character not owned
        API-->>C: 401 Unauthorized
    else Owned
        API-->>C: 200 OK
    end

    C->>API: DELETE /characters/{id} (Authorization)
    API->>API: verify_token(...)
    alt Character not owned
        API-->>C: 401 Unauthorized
    else Owned
        API-->>C: 200 OK
    end
```