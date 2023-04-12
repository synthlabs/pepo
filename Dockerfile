FROM denoland/deno:latest

# The port that your application listens to.
EXPOSE 3000

WORKDIR /app

# Prefer not to run as root.
USER deno

# Cache the dependencies as a layer (the following two steps are re-run only when deps.ts is modified).
# Ideally cache deps.ts will download and compile _all_ external files used in index.js.
COPY deps.ts .
RUN deno cache deps.ts

# These steps will be re-run upon each file change in your working directory:
COPY . .
# Compile the index app so that it doesn't need to be compiled each startup/entry.
RUN deno cache index.js

CMD ["run", "--allow-all", "index.js"]
