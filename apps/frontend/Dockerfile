# Stage 1: Build the React frontend
FROM docker.io/library/node:22-alpine@sha256:c610fcdfb1d5b4740dd70c284ed3cb16bb857e0f7166196e36a5501df7a3aa32 AS builder

WORKDIR /app

COPY package.json package-lock.json ./
COPY scripts/ ./scripts/
RUN npm ci

COPY . .
RUN npm run build

# Stage 2: Serve with nginx
FROM docker.io/library/nginx:alpine@sha256:4a73073bd557c65b759505da037898b61f1be6cbcc3c2c3aeac22d2a470c1752

COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80
