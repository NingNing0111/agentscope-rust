//! Tests for the EmbeddingModel trait using a mock implementation.

use agent_scope_embedding::{
    EmbeddingError, EmbeddingInput, EmbeddingModel, EmbeddingModelCard, EmbeddingResponse,
    EmbeddingUsage,
};

/// A mock embedding model that returns fixed-dimension vectors.
struct MockEmbeddingModel {
    card: EmbeddingModelCard,
}

impl MockEmbeddingModel {
    fn new(dimensions: u32, supports_multimodal: bool) -> Self {
        Self {
            card: EmbeddingModelCard::new("mock-model", dimensions, supports_multimodal),
        }
    }
}

#[async_trait::async_trait]
impl EmbeddingModel for MockEmbeddingModel {
    async fn embed(
        &self,
        inputs: Vec<EmbeddingInput>,
    ) -> Result<EmbeddingResponse, EmbeddingError> {
        // Check for unsupported DataBlock inputs
        if !self.supports_multimodal() {
            for input in &inputs {
                if matches!(input, EmbeddingInput::DataBlock(_)) {
                    return Err(EmbeddingError::MultimodalNotSupported);
                }
            }
        }

        let dim = self.card.dimensions as usize;
        let embeddings: Vec<Vec<f32>> = inputs
            .iter()
            .enumerate()
            .map(|(i, _)| vec![i as f32; dim])
            .collect();
        let tokens = inputs.len() as u32 * 10;

        Ok(EmbeddingResponse {
            embeddings,
            usage: EmbeddingUsage {
                total_tokens: tokens,
            },
        })
    }

    fn model_card(&self) -> &EmbeddingModelCard {
        &self.card
    }
}

#[tokio::test]
async fn test_mock_embed_basic() {
    let model = MockEmbeddingModel::new(4, false);
    let inputs: Vec<EmbeddingInput> = vec!["hello".into(), "world".into()];
    let response = model.embed(inputs).await.expect("embed should succeed");

    assert_eq!(response.embeddings.len(), 2);
    assert_eq!(response.embeddings[0].len(), 4);
    assert_eq!(response.embeddings[1].len(), 4);
    assert_eq!(response.embeddings[0], vec![0.0_f32; 4]);
    assert_eq!(response.embeddings[1], vec![1.0_f32; 4]);
    assert!(response.usage.total_tokens > 0);
}

#[tokio::test]
async fn test_mock_model_card() {
    let model = MockEmbeddingModel::new(8, true);
    let card = model.model_card();

    assert_eq!(card.dimensions, 8);
    assert!(card.supports_multimodal);
}

#[tokio::test]
async fn test_mock_supports_multimodal_default() {
    let model = MockEmbeddingModel::new(4, false);
    assert!(!model.supports_multimodal());

    let model2 = MockEmbeddingModel::new(4, true);
    assert!(model2.supports_multimodal());
}

#[tokio::test]
async fn test_mock_datablock_rejected_when_not_multimodal() {
    let model = MockEmbeddingModel::new(4, false);
    let inputs = vec![EmbeddingInput::DataBlock("image_data".into())];
    let result = model.embed(inputs).await;

    assert!(result.is_err());
    assert!(matches!(
        result.expect_err("should error"),
        EmbeddingError::MultimodalNotSupported
    ));
}

#[tokio::test]
async fn test_mock_datablock_accepted_when_multimodal() {
    let model = MockEmbeddingModel::new(4, true);
    let inputs = vec![EmbeddingInput::DataBlock("image_data".into())];
    let response = model.embed(inputs).await.expect("embed should succeed");

    assert_eq!(response.embeddings.len(), 1);
    assert_eq!(response.embeddings[0].len(), 4);
}
