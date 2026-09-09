export interface MessageEnvelope {
    event_type: string;
    api_version: number;
    request_id?: string;
    transaction_id?: string;
    retry_count: number;
    created_at: number;
    data: any;
}

export enum RequestPriority {
    High = 0,   // Immediate UI actions (e.g. Inspector edit)
    Normal = 1, // Standard sync
    Low = 2     // Telemetry, Background tasks
}

export interface PendingRequest {
    envelope: MessageEnvelope;
    priority: RequestPriority;
    expiration: number; // timestamp when to drop it
    resolve?: (value: any) => void;
    reject?: (reason?: any) => void;
}

export class PendingRequestQueue {
    private queue: PendingRequest[] = [];
    private readonly defaultTtlMs = 60000; // 1 dakika

    public enqueue(envelope: MessageEnvelope, priority: RequestPriority = RequestPriority.Normal): Promise<any> {
        return new Promise((resolve, reject) => {
            const request: PendingRequest = {
                envelope,
                priority,
                expiration: Date.now() + this.defaultTtlMs,
                resolve,
                reject
            };

            this.queue.push(request);
            // Sort by priority (0 is highest) and then by creation time
            this.queue.sort((a, b) => {
                if (a.priority === b.priority) {
                    return a.envelope.created_at - b.envelope.created_at;
                }
                return a.priority - b.priority;
            });
        });
    }

    public dequeueAllValid(): PendingRequest[] {
        const now = Date.now();
        const validRequests: PendingRequest[] = [];
        const expiredRequests: PendingRequest[] = [];

        for (const req of this.queue) {
            if (now > req.expiration) {
                expiredRequests.push(req);
            } else {
                validRequests.push(req);
            }
        }

        // Expired olanları reject et
        for (const exp of expiredRequests) {
            if (exp.reject) {
                exp.reject(new Error("Request TTL expired in queue."));
            }
        }

        this.queue = [];
        return validRequests;
    }

    public removeByRequestId(requestId: string) {
        this.queue = this.queue.filter(req => req.envelope.request_id !== requestId);
    }
}
