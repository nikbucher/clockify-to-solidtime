use std::{
	error::Error,
	fmt,
	sync::Mutex,
	thread,
	time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use reqwest::{
	StatusCode,
	blocking::{Client, RequestBuilder, Response},
};
use serde::de::DeserializeOwned;

#[derive(Debug)]
pub struct HttpStatusError {
	status: StatusCode,
	body: String,
}

impl HttpStatusError {
	pub fn status(&self) -> StatusCode {
		self.status
	}
}

impl fmt::Display for HttpStatusError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(formatter, "HTTP {}: {}", self.status, self.body)
	}
}

impl Error for HttpStatusError {}

pub struct HttpClient {
	client: Client,
	last_request: Mutex<Option<Instant>>,
	min_interval: Duration,
}

impl HttpClient {
	pub fn new() -> Result<Self> {
		let client = Client::builder()
			.user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
			.connect_timeout(Duration::from_secs(10))
			.timeout(Duration::from_secs(60))
			.build()
			.context("failed to build HTTP client")?;
		Ok(Self {
			client,
			last_request: Mutex::new(None),
			min_interval: Duration::from_millis(110),
		})
	}

	pub fn client(&self) -> &Client {
		&self.client
	}

	pub fn send_json<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<T> {
		let response = self.send(request)?;
		response.json::<T>().context("failed to deserialize JSON response")
	}

	pub fn send(&self, request: RequestBuilder) -> Result<Response> {
		let mut current = Some(request);
		let mut delay = Duration::from_millis(400);

		for attempt in 0..5 {
			self.throttle();
			let request = current.take().expect("request should be available");
			let retryable = request.try_clone();
			let response = request.send();

			match response {
				Ok(response) if response.status().is_success() => return Ok(response),
				Ok(response) if is_retryable(response.status()) && attempt < 4 => {
					current = retryable;
					if current.is_none() {
						return Err(anyhow!("retryable response cannot be retried because the request cannot be cloned"));
					}
					sleep_retry(delay);
					delay *= 2;
				}
				Ok(response) => {
					let status = response.status();
					let body = response.text().unwrap_or_default();
					return Err(HttpStatusError { status, body }.into());
				}
				Err(err) if attempt < 4 => {
					current = retryable;
					sleep_retry(delay);
					delay *= 2;
					if current.is_none() {
						return Err(err).context("request failed and cannot be retried");
					}
				}
				Err(err) => return Err(err).context("request failed"),
			}
		}

		Err(anyhow!("request failed after retries"))
	}

	fn throttle(&self) {
		let mut last_request = self.last_request.lock().expect("rate limiter mutex poisoned");
		if let Some(last) = *last_request {
			let elapsed = last.elapsed();
			if elapsed < self.min_interval {
				thread::sleep(self.min_interval - elapsed);
			}
		}
		*last_request = Some(Instant::now());
	}
}

fn is_retryable(status: StatusCode) -> bool {
	status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn sleep_retry(delay: Duration) {
	thread::sleep(delay);
}

pub fn error_status(error: &anyhow::Error) -> Option<StatusCode> {
	error.chain().find_map(|cause| cause.downcast_ref::<HttpStatusError>().map(HttpStatusError::status))
}
