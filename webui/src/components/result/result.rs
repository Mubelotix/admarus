use crate::prelude::*;

#[derive(Clone, Properties)]
pub struct GroupedResultsProps {
    pub results: Vec<(DocumentResult, Scores)>,
    pub query: Rc<Query>,
}

// TODO: revise this
impl PartialEq for GroupedResultsProps {
    fn eq(&self, other: &Self) -> bool {
        if self.results.len() != other.results.len() {
            return false;
        }
        if self.query != other.query {
            return false;
        }
        for i in 0..self.results.len() {
            if self.results[i].0.cid != other.results[i].0.cid {
                return false;
            }
            if self.results[i].1 != other.results[i].1 {
                return false;
            }
        }
        true
    }
}

pub struct GroupedResultsComp {

}

impl Component for GroupedResultsComp {
    type Message = ();
    type Properties = GroupedResultsProps;

    fn create(_ctx: &Context<Self>) -> Self {
        GroupedResultsComp {}
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        *self = Component::create(ctx);
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if ctx.props().results.is_empty() {
            return html! {};
        }
        let has_children = ctx.props().results.len() > 1;
        let root_id = ctx.props().results.first().map(|(result,_)| result.root_id()).unwrap_or_default();

        let mut addr_iter = ctx.props().results.iter().map(|(result,_)| result.format_best_addr());
        let mut href_iter = ctx.props().results.iter().map(|(result,_)| result.format_best_href());
        let mut title_iter = ctx.props().results.iter().map(|(result,_)| result.format_result_title());
        let mut desc_iter = ctx.props().results.iter().map(|(result,_)| result.view_desc(&ctx.props().query));

        // Scores
        let display_scores = false; // cfg!(debug_assertions);
        let mut term_frequency_score_iter = ctx.props().results.iter().map(|(_, scores)| scores.tf_score);
        let mut variety_score_iter = ctx.props().results.iter().map(|(_, scores)| scores.variety_score);
        let mut length_score_iter = ctx.props().results.iter().map(|(_, scores)| scores.length_score);
        let mut lang_score_iter = ctx.props().results.iter().map(|(_, scores)| scores.lang_score);
        let mut popularity_score_iter = ctx.props().results.iter().map(|(_, scores)| scores.popularity_score);
        let mut verified_score_iter = ctx.props().results.iter().map(|(_, scores)| scores.verified_score);
        
        let href_first = href_iter.next().unwrap_or_default();
        let title_first = title_iter.next().unwrap_or_default();
        let desc_first = desc_iter.next().unwrap_or_default();
        let addr_first = addr_iter.next().unwrap_or_default();
        let term_frequency_score_first = term_frequency_score_iter.next().unwrap();
        let variety_score_first = variety_score_iter.next().unwrap();
        let length_score_first = length_score_iter.next().unwrap();
        let lang_score_first = lang_score_iter.next().unwrap();
        let popularity_score_first = popularity_score_iter.next().unwrap();
        let verified_score_first = verified_score_iter.next().unwrap();

        template_html!("components/result/result.html", ...)
    }
}
